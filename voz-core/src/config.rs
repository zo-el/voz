// SPDX-License-Identifier: Apache-2.0
//! Persisted settings (`~/.config/voz/config.toml`) with schema versioning.
//!
//! Secrets (e.g. a Claude API key) are NOT stored here — they live in the OS
//! secret service (see `docs/SECURITY.md §2.3`). This struct only records which
//! backend is selected.

use crate::model::{RefineStyle, Source};
use serde::{Deserialize, Serialize};

/// Current on-disk schema version; bump when the shape changes and add a
/// migration in [`Settings::migrate`].
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefineBackend {
    None,
    ClaudeCode,
    Codex,
    Ollama,
    ClaudeApi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accel {
    Auto,
    Cpu,
    Vulkan,
    Cuda,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneralCfg {
    pub save_dir: String,
    pub format: String,
    pub keep_audio: bool,
    pub obsidian_links: bool,
    pub theme: String,
    pub launch_on_login: bool,
    /// Whether the first-run onboarding has been completed. `#[serde(default)]` so
    /// configs written before this field load cleanly (treated as not onboarded).
    #[serde(default)]
    pub onboarded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourcesCfg {
    pub mic_device: String,
    pub system_audio: bool,
    pub default_source: Source,
    pub attribute: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptionCfg {
    pub model: String,
    pub accel: Accel,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefineCfg {
    pub backend: RefineBackend,
    pub ollama_model: String,
    pub style: RefineStyle,
    pub lossless_guard: bool,
}

/// Top-level settings, matching the schema in `docs/ARCHITECTURE.md §8`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    pub general: GeneralCfg,
    pub sources: SourcesCfg,
    pub transcription: TranscriptionCfg,
    pub refine: RefineCfg,
}

fn default_schema() -> u32 {
    SCHEMA_VERSION
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema_version: SCHEMA_VERSION,
            general: GeneralCfg {
                save_dir: "~/Recordings/Voz".into(),
                format: "md".into(),
                keep_audio: true,
                obsidian_links: true,
                theme: "dark".into(),
                launch_on_login: true,
                onboarded: false,
            },
            sources: SourcesCfg {
                mic_device: "default".into(),
                system_audio: true,
                default_source: Source::Both,
                attribute: true,
            },
            transcription: TranscriptionCfg {
                model: "large-v3-turbo-q5_0".into(),
                accel: Accel::Auto,
                language: "auto".into(),
            },
            refine: RefineCfg {
                backend: RefineBackend::ClaudeCode,
                ollama_model: "qwen2.5:3b".into(),
                style: RefineStyle::Adaptive,
                lossless_guard: true,
            },
        }
    }
}

impl Settings {
    /// Parse from a TOML string, applying migrations for older schema versions.
    ///
    /// # Errors
    /// Returns [`crate::Error::Config`] if the TOML is malformed.
    pub fn from_toml(s: &str) -> crate::Result<Self> {
        let mut cfg: Settings =
            toml::from_str(s).map_err(|e| crate::Error::Config(e.to_string()))?;
        cfg.migrate();
        Ok(cfg)
    }

    /// Serialize to a TOML string.
    ///
    /// # Errors
    /// Returns [`crate::Error::Config`] if serialization fails.
    pub fn to_toml(&self) -> crate::Result<String> {
        toml::to_string_pretty(self).map_err(|e| crate::Error::Config(e.to_string()))
    }

    /// Apply forward migrations so an older config loads cleanly. New cases are
    /// added here as `SCHEMA_VERSION` increases.
    pub fn migrate(&mut self) {
        // v0/unknown -> v1: nothing structural yet; just stamp the version.
        if self.schema_version < SCHEMA_VERSION {
            self.schema_version = SCHEMA_VERSION;
        }
    }

    /// `$XDG_CONFIG_HOME/voz/config.toml` (fallback `~/.config/voz/config.toml`).
    #[must_use]
    pub fn config_path() -> std::path::PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
            })
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        base.join("voz").join("config.toml")
    }

    /// Load settings from disk, or defaults if absent/unreadable (a corrupt file
    /// is backed up rather than crashing).
    #[must_use]
    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(s) => match Self::from_toml(&s) {
                Ok(cfg) => cfg,
                Err(_) => {
                    let _ = std::fs::rename(&path, path.with_extension("toml.bak"));
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Persist settings to `config_path()` (atomic temp+rename).
    ///
    /// # Errors
    /// Returns [`crate::Error::Config`] on a write failure.
    pub fn save(&self) -> crate::Result<()> {
        let path = Self::config_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let toml = self.to_toml()?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, toml)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_round_trips_through_toml() {
        let s = Settings::default();
        let toml = s.to_toml().unwrap();
        let back = Settings::from_toml(&toml).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn defaults_match_locked_decisions() {
        let s = Settings::default();
        assert_eq!(s.sources.default_source, Source::Both);
        assert_eq!(s.refine.backend, RefineBackend::ClaudeCode);
        assert_eq!(s.refine.style, RefineStyle::Adaptive);
        assert_eq!(s.general.theme, "dark");
        assert!(s.general.obsidian_links);
    }

    #[test]
    fn missing_schema_version_defaults_and_migrates() {
        // A hand-written config without schema_version should still load.
        let toml = r#"
            [general]
            save_dir = "~/vault"
            format = "md"
            keep_audio = true
            obsidian_links = true
            theme = "dark"
            launch_on_login = false
            [sources]
            mic_device = "default"
            system_audio = true
            default_source = "both"
            attribute = true
            [transcription]
            model = "small"
            accel = "auto"
            language = "auto"
            [refine]
            backend = "claude_code"
            ollama_model = "qwen2.5:3b"
            style = "adaptive"
            lossless_guard = true
        "#;
        let cfg = Settings::from_toml(toml).unwrap();
        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
        assert_eq!(cfg.general.save_dir, "~/vault");
    }

    #[test]
    fn malformed_toml_is_a_config_error() {
        assert!(matches!(
            Settings::from_toml("not = [valid"),
            Err(crate::Error::Config(_))
        ));
    }

    #[test]
    fn save_then_load_round_trips_on_disk() {
        let home = std::env::temp_dir().join(format!("voz-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &home);
        let mut s = Settings::default();
        s.general.save_dir = "~/Obsidian/MyVault/Voz".into();
        s.sources.default_source = Source::Mic;
        s.save().unwrap();
        let loaded = Settings::load();
        assert_eq!(loaded.general.save_dir, "~/Obsidian/MyVault/Voz");
        assert_eq!(loaded.sources.default_source, Source::Mic);
        let _ = std::fs::remove_dir_all(&home);
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
