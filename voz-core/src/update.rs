// SPDX-License-Identifier: Apache-2.0
//! Version comparison for the in-app update check. Pure (no network here) so it's
//! unit-tested; the app does the GitHub Releases fetch and feeds the tag in.

/// Parse a release tag like `v1.2.3` / `1.2` / `2.0.0-rc1` into `(major, minor,
/// patch)`. Missing components default to 0; a leading `v` is ignored.
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

/// True if `candidate` is a strictly newer version than `current`. Unparseable
/// inputs are treated as "not newer" (never nags on garbage).
#[must_use]
pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (parse(current), parse(candidate)) {
        (Some(c), Some(n)) => n > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_detected_across_components() {
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.9.9", "1.0.0"));
        assert!(is_newer("v0.1.0", "v0.1.1")); // leading v tolerated
        assert!(is_newer("0.1.0", "0.1.1-rc1")); // pre-release suffix ignored in core
    }

    #[test]
    fn same_or_older_is_not_newer() {
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.2.0", "0.1.9"));
        assert!(!is_newer("1.0.0", "0.9.9"));
    }

    #[test]
    fn missing_components_default_to_zero() {
        assert!(is_newer("0.1", "0.1.1"));
        assert!(!is_newer("0.1", "0.1.0"));
        assert!(is_newer("1", "1.0.1"));
    }

    #[test]
    fn garbage_never_nags() {
        assert!(!is_newer("0.1.0", "not-a-version"));
        assert!(!is_newer("", "0.1.0"));
        assert!(!is_newer("0.1.0", ""));
    }
}
