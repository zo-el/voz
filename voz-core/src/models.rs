// SPDX-License-Identifier: Apache-2.0
//! Whisper model registry, on-disk cache, and verified downloads (feature `whisper`).
//!
//! Models are GGML files from the official Hugging Face `ggerganov/whisper.cpp`
//! repo. The default model is bundled with the app (zero-setup); other sizes are
//! optional, **consented**, **checksum-verified** downloads (see
//! `docs/SECURITY.md §2.4`). Downloads stream to a temp file, are SHA-256 verified,
//! then atomically renamed in.

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;

/// A downloadable model and its expected integrity hash.
#[derive(Debug, Clone, Copy)]
pub struct ModelInfo {
    pub id: &'static str,
    pub display: &'static str,
    pub url: &'static str,
    /// Expected lowercase hex SHA-256. Empty = not yet pinned (download refused
    /// unless `allow_unverified` — keeps us honest about which models are vetted).
    pub sha256: &'static str,
    pub size_mb: u32,
}

const HF: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// The vetted model registry. Hashes for `tiny.en`/`base.en` are pinned (verified
/// in CI's slow lane); larger entries are pinned as they're added.
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "tiny.en",
        display: "Tiny (English)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
        sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
        size_mb: 75,
    },
    ModelInfo {
        id: "base.en",
        display: "Base (English)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
        sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
        size_mb: 142,
    },
    ModelInfo {
        id: "small",
        display: "Small (multilingual)",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        sha256: "", // pinned when added to the bundle/QA
        size_mb: 466,
    },
    ModelInfo {
        id: "large-v3-turbo-q5_0",
        display: "Large v3 Turbo (q5_0)",
        url:
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        sha256: "",
        size_mb: 547,
    },
];

#[must_use]
pub fn lookup(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

/// `$XDG_DATA_HOME/voz/models` (fallback `~/.local/share/voz/models`).
#[must_use]
pub fn models_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("voz").join("models")
}

#[must_use]
pub fn model_path(id: &str) -> PathBuf {
    models_dir().join(format!("ggml-{id}.bin"))
}

#[must_use]
pub fn is_installed(id: &str) -> bool {
    model_path(id).is_file()
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Compute the SHA-256 of a file as lowercase hex.
///
/// # Errors
/// Propagates I/O errors.
pub fn file_sha256(path: &std::path::Path) -> crate::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// Download model `id` to the cache, verifying its SHA-256. `progress(done, total)`
/// is called as bytes arrive (`total` is 0 if the server omits Content-Length).
///
/// # Errors
/// Fails on network errors, a checksum mismatch, or an unpinned model when
/// `allow_unverified` is false.
pub fn download(
    id: &str,
    allow_unverified: bool,
    mut progress: impl FnMut(u64, u64),
) -> crate::Result<PathBuf> {
    let info = lookup(id).ok_or_else(|| crate::Error::Storage(format!("unknown model {id}")))?;
    if info.sha256.is_empty() && !allow_unverified {
        return Err(crate::Error::Storage(format!(
            "model {id} has no pinned checksum; refusing to download (security)"
        )));
    }
    let _ = HF; // base URL documented above; entries carry full URLs.
    let dir = models_dir();
    std::fs::create_dir_all(&dir)?;
    let final_path = model_path(id);
    let tmp = dir.join(format!("ggml-{id}.bin.part"));

    let resp = ureq::get(info.url)
        .call()
        .map_err(|e| crate::Error::Storage(format!("download {id}: {e}")))?;
    let total: u64 = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&tmp)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    let mut done: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
        done += n as u64;
        progress(done, total);
    }
    file.sync_all()?;
    drop(file);

    let digest = hex(&hasher.finalize());
    if !info.sha256.is_empty() && digest != info.sha256 {
        let _ = std::fs::remove_file(&tmp);
        return Err(crate::Error::Storage(format!(
            "checksum mismatch for {id}: got {digest}, expected {}",
            info.sha256
        )));
    }
    std::fs::rename(&tmp, &final_path)?;
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lookup_and_paths() {
        assert!(lookup("base.en").is_some());
        assert!(lookup("nope").is_none());
        assert!(model_path("base.en").ends_with("ggml-base.en.bin"));
        assert!(models_dir().ends_with("voz/models"));
    }

    #[test]
    fn unpinned_model_refused_by_default() {
        let err = download("small", false, |_, _| {});
        assert!(matches!(err, Err(crate::Error::Storage(_))));
    }

    #[test]
    fn hex_encoding() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
    }
}
