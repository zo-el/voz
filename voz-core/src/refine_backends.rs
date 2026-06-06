// SPDX-License-Identifier: Apache-2.0
//! Concrete refine backends (feature `refine`): Claude Code CLI (default),
//! Codex CLI, Ollama (local HTTP), and the Claude API (BYO key).
//!
//! Security (see `docs/SECURITY.md §2.2`): the transcript is **untrusted data**.
//! CLI backends pass it on **stdin** (never as a shell argument and never via
//! `sh -c`), with a process timeout and a bounded output read. The model's output
//! is only ever written to a note — it is never executed.

use crate::config::{RefineBackend, RefineCfg};
use crate::model::{RefineStyle, Transcript};
use crate::refine::{build_input, Refiner};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

/// Hard limits applied to every backend.
const CLI_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// True if `bin` is an executable on `$PATH` (used to detect CLI backends without
/// spawning them).
#[must_use]
pub fn cli_on_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file()))
}

/// Whether the selected backend can actually run right now (CLI installed / key
/// set). Lets the app fall back gracefully instead of failing every recording.
#[must_use]
pub fn backend_available(cfg: &RefineCfg, has_api_key: bool) -> bool {
    match cfg.backend {
        RefineBackend::None | RefineBackend::Ollama => true, // Ollama checked at call time
        RefineBackend::ClaudeCode => cli_on_path("claude"),
        RefineBackend::Codex => cli_on_path("codex"),
        RefineBackend::ClaudeApi => has_api_key,
    }
}

/// Build the right refiner from settings. Returns `None` for `RefineBackend::None`
/// (raw-only) — the pipeline then saves just the raw note.
#[must_use]
pub fn build_refiner(cfg: &RefineCfg, api_key: Option<String>) -> Option<Box<dyn Refiner>> {
    match cfg.backend {
        RefineBackend::None => None,
        RefineBackend::ClaudeCode => Some(Box::new(CliRefiner::claude_code())),
        RefineBackend::Codex => Some(Box::new(CliRefiner::codex())),
        RefineBackend::Ollama => Some(Box::new(OllamaRefiner::new(cfg.ollama_model.clone()))),
        RefineBackend::ClaudeApi => {
            Some(Box::new(ClaudeApiRefiner::new(api_key.unwrap_or_default())))
        }
    }
}

/// Run `bin args...`, writing `stdin_data` to the child's stdin and returning its
/// stdout. Drains stdout on a thread (no pipe-deadlock), enforces a timeout, and
/// caps the captured output.
fn run_cli(bin: &str, args: &[&str], stdin_data: &str) -> crate::Result<String> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| crate::Error::Refine(format!("{bin}: {e}")))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| crate::Error::Refine("no stdin".into()))?;
    let data = stdin_data.to_string();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(data.as_bytes());
        // drop closes stdin so the child sees EOF
    });

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| crate::Error::Refine("no stdout".into()))?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout
            .by_ref()
            .take(MAX_OUTPUT_BYTES as u64)
            .read_to_end(&mut buf);
        buf
    });

    let status = match child
        .wait_timeout(CLI_TIMEOUT)
        .map_err(|e| crate::Error::Refine(e.to_string()))?
    {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let _ = writer.join();
            let _ = reader.join();
            return Err(crate::Error::Refine(format!(
                "{bin} timed out after {CLI_TIMEOUT:?}"
            )));
        }
    };
    let _ = writer.join();
    let out = reader.join().unwrap_or_default();
    if !status.success() {
        return Err(crate::Error::Refine(format!(
            "{bin} exited unsuccessfully ({status})"
        )));
    }
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// A CLI-based backend (Claude Code or Codex). The fixed instruction goes in the
/// argv prompt; the transcript (plus the delimited prompt) is piped on stdin.
pub struct CliRefiner {
    display: &'static str,
    bin: String,
    prompt_args: Vec<String>,
}

impl std::fmt::Debug for CliRefiner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliRefiner")
            .field("bin", &self.bin)
            .finish()
    }
}

impl CliRefiner {
    #[must_use]
    pub fn claude_code() -> Self {
        CliRefiner {
            display: "Claude Code",
            bin: "claude".into(),
            // -p = headless print mode; the instruction is trusted, transcript is stdin.
            prompt_args: vec![
                "-p".into(),
                "Follow the instructions in the input and output only the resulting note, nothing else.".into(),
            ],
        }
    }

    #[must_use]
    pub fn codex() -> Self {
        CliRefiner {
            display: "Codex",
            bin: "codex".into(),
            prompt_args: vec![
                "exec".into(),
                "Follow the instructions in the input and output only the resulting note, nothing else.".into(),
            ],
        }
    }
}

impl Refiner for CliRefiner {
    fn name(&self) -> &str {
        self.display
    }
    fn refine(&self, raw: &Transcript, style: &RefineStyle) -> crate::Result<String> {
        let args: Vec<&str> = self.prompt_args.iter().map(String::as_str).collect();
        let out = run_cli(&self.bin, &args, &build_input(raw, style))?;
        if out.is_empty() {
            return Err(crate::Error::Refine(format!(
                "{} returned empty output",
                self.display
            )));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// HTTP backends
// ---------------------------------------------------------------------------

/// Local Ollama (`http://localhost:11434`). Fully offline.
#[derive(Debug)]
pub struct OllamaRefiner {
    model: String,
    endpoint: String,
}

impl OllamaRefiner {
    #[must_use]
    pub fn new(model: String) -> Self {
        OllamaRefiner {
            model,
            endpoint: "http://localhost:11434/api/generate".into(),
        }
    }
}

/// Build the Ollama `/api/generate` request body (pure; unit-tested).
#[must_use]
pub fn ollama_body(model: &str, prompt: &str) -> serde_json::Value {
    serde_json::json!({ "model": model, "prompt": prompt, "stream": false })
}

impl Refiner for OllamaRefiner {
    fn name(&self) -> &str {
        "Local LLM"
    }
    fn refine(&self, raw: &Transcript, style: &RefineStyle) -> crate::Result<String> {
        let body = ollama_body(&self.model, &build_input(raw, style));
        let resp = ureq::post(&self.endpoint)
            .send_json(body)
            .map_err(|e| crate::Error::Refine(format!("ollama: {e}")))?;
        let json: serde_json::Value = resp
            .into_json()
            .map_err(|e| crate::Error::Refine(e.to_string()))?;
        let text = json
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if text.is_empty() {
            return Err(crate::Error::Refine("ollama returned no text".into()));
        }
        Ok(text)
    }
}

/// Anthropic Messages API (bring your own key). The key is read from the OS secret
/// service by the app and passed in here — never logged, never persisted in config.
#[derive(Debug)]
pub struct ClaudeApiRefiner {
    api_key: String,
    model: String,
    endpoint: String,
}

impl ClaudeApiRefiner {
    #[must_use]
    pub fn new(api_key: String) -> Self {
        ClaudeApiRefiner {
            api_key,
            model: "claude-sonnet-4-6".into(),
            endpoint: "https://api.anthropic.com/v1/messages".into(),
        }
    }
}

/// Build the Anthropic Messages request body: fixed prompt as `system`, transcript
/// as the user message (pure; unit-tested).
#[must_use]
pub fn claude_api_body(model: &str, system: &str, transcript: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "system": system,
        "messages": [{ "role": "user", "content": transcript }],
    })
}

impl Refiner for ClaudeApiRefiner {
    fn name(&self) -> &str {
        "Claude API"
    }
    fn refine(&self, raw: &Transcript, style: &RefineStyle) -> crate::Result<String> {
        if self.api_key.is_empty() {
            return Err(crate::Error::Refine("Claude API key not set".into()));
        }
        let body = claude_api_body(&self.model, &style.prompt(), &raw.plain_text());
        let resp = ureq::post(&self.endpoint)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(|e| crate::Error::Refine(format!("claude api: {e}")))?;
        let json: serde_json::Value = resp
            .into_json()
            .map_err(|e| crate::Error::Refine(e.to_string()))?;
        let text = json
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if text.is_empty() {
            return Err(crate::Error::Refine("claude api returned no text".into()));
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Speaker, Turn};

    fn tr(text: &str) -> Transcript {
        Transcript {
            turns: vec![Turn {
                speaker: Speaker::Me,
                text: text.into(),
                start_ms: 0,
                end_ms: 0,
            }],
            language: None,
        }
    }

    #[test]
    fn factory_maps_backends() {
        let mut cfg = RefineCfg {
            backend: RefineBackend::None,
            ollama_model: "x".into(),
            style: RefineStyle::Adaptive,
            lossless_guard: true,
        };
        assert!(build_refiner(&cfg, None).is_none());
        cfg.backend = RefineBackend::ClaudeCode;
        assert_eq!(build_refiner(&cfg, None).unwrap().name(), "Claude Code");
        cfg.backend = RefineBackend::Ollama;
        assert_eq!(build_refiner(&cfg, None).unwrap().name(), "Local LLM");
        cfg.backend = RefineBackend::ClaudeApi;
        assert_eq!(
            build_refiner(&cfg, Some("k".into())).unwrap().name(),
            "Claude API"
        );
    }

    #[test]
    fn run_cli_pipes_stdin_without_a_shell() {
        // `cat` echoes stdin verbatim — proving the data goes via stdin, not argv,
        // and shell metacharacters are inert (no `sh -c`).
        let payload = "hello; rm -rf / `echo pwned` $(whoami)";
        let out = run_cli("cat", &[], payload).unwrap();
        assert_eq!(out, payload);
    }

    #[test]
    fn run_cli_reports_nonzero_exit() {
        let err = run_cli("false", &[], "");
        assert!(matches!(err, Err(crate::Error::Refine(_))));
    }

    #[test]
    fn ollama_body_shape() {
        let b = ollama_body("qwen2.5:3b", "do it");
        assert_eq!(b["model"], "qwen2.5:3b");
        assert_eq!(b["stream"], false);
        assert_eq!(b["prompt"], "do it");
    }

    #[test]
    fn claude_api_body_puts_transcript_in_user_message() {
        let b = claude_api_body("claude-sonnet-4-6", "system prompt", "the transcript");
        assert_eq!(b["system"], "system prompt");
        assert_eq!(b["messages"][0]["role"], "user");
        assert_eq!(b["messages"][0]["content"], "the transcript");
    }

    #[test]
    fn claude_api_requires_key() {
        let r = ClaudeApiRefiner::new(String::new());
        assert!(r.refine(&tr("x"), &RefineStyle::Adaptive).is_err());
    }

    #[test]
    fn cli_detection_and_availability() {
        assert!(cli_on_path("sh")); // present on any unix
        assert!(!cli_on_path("definitely-not-a-real-binary-xyz"));
        let mut cfg = RefineCfg {
            backend: RefineBackend::None,
            ollama_model: String::new(),
            style: RefineStyle::Adaptive,
            lossless_guard: true,
        };
        assert!(backend_available(&cfg, false)); // None always available
        cfg.backend = RefineBackend::ClaudeApi;
        assert!(!backend_available(&cfg, false)); // no key
        assert!(backend_available(&cfg, true)); // key present
    }
}
