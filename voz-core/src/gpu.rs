// SPDX-License-Identifier: Apache-2.0
//! GPU/acceleration detection for the "what am I actually using" indicator.
//!
//! A binary can only use a backend it was **compiled** with ([`build_backend`]);
//! at runtime we probe whether a usable device is present and describe the
//! *effective* backend the app will run on. The probe is best-effort (shells to
//! `nvidia-smi` / checks `/dev/dri`); the mapping is pure and unit-tested.

/// The acceleration backend this binary was built with.
#[must_use]
pub fn build_backend() -> &'static str {
    if cfg!(feature = "cuda") {
        "cuda"
    } else if cfg!(feature = "vulkan") {
        "vulkan"
    } else {
        "cpu"
    }
}

/// Best-effort: is an NVIDIA GPU usable (driver loaded)?
#[must_use]
pub fn nvidia_present() -> bool {
    if std::path::Path::new("/proc/driver/nvidia/version").exists() {
        return true;
    }
    std::process::Command::new("nvidia-smi")
        .arg("-L")
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Best-effort: is any DRM render node present (a GPU Vulkan/compute could use)?
#[must_use]
pub fn dri_present() -> bool {
    std::fs::read_dir("/dev/dri")
        .map(|d| {
            d.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("renderD"))
        })
        .unwrap_or(false)
}

/// Pure mapping from (compiled backend, detected devices) to a human description of
/// the *effective* backend. Separated out so it's testable without real hardware.
#[must_use]
pub fn describe(build: &str, nvidia: bool, dri: bool) -> String {
    match build {
        "cuda" if nvidia => "CUDA — NVIDIA GPU".into(),
        "cuda" => "CPU — built for CUDA, but no NVIDIA GPU detected".into(),
        "vulkan" if dri => "Vulkan — GPU".into(),
        "vulkan" => "CPU — built for Vulkan, but no GPU detected".into(),
        _ => "CPU".into(),
    }
}

/// The effective backend description for the running binary.
#[must_use]
pub fn effective_backend_desc() -> String {
    describe(build_backend(), nvidia_present(), dri_present())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_covers_each_case() {
        assert_eq!(describe("cuda", true, true), "CUDA — NVIDIA GPU");
        assert!(describe("cuda", false, true).starts_with("CPU"));
        assert_eq!(describe("vulkan", false, true), "Vulkan — GPU");
        assert!(describe("vulkan", false, false).starts_with("CPU"));
        assert_eq!(describe("cpu", true, true), "CPU");
    }

    #[test]
    fn build_backend_is_known() {
        assert!(matches!(build_backend(), "cpu" | "cuda" | "vulkan"));
    }

    #[test]
    fn probes_never_panic() {
        let _ = nvidia_present();
        let _ = dri_present();
        let _ = effective_backend_desc();
    }
}
