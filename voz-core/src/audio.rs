// SPDX-License-Identifier: Apache-2.0
//! Audio types and pure-DSP helpers (no device I/O here).
//!
//! Capture from the mic and the PipeWire sink **monitor** (loopback) is added
//! behind the `audio` feature in a later milestone. The conversion helpers below
//! — downmix and i16→f32 — are pure and fully tested now; resampling to 16 kHz
//! will use `rubato` once the native feature is enabled.

/// Running peak/RMS level (0.0–1.0) for the live waveform, per source.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Level {
    pub mic: f32,
    pub system: f32,
}

/// Downmix interleaved samples to mono by averaging channels.
///
/// `channels` must be ≥ 1. Returns one sample per frame.
#[must_use]
pub fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Convert signed 16-bit PCM to normalized f32 in [-1.0, 1.0].
#[must_use]
pub fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| f32::from(s) / 32768.0).collect()
}

/// Root-mean-square level of a mono buffer, clamped to [0.0, 1.0].
#[must_use]
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt().clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_stereo() {
        let stereo = [1.0, 0.0, 0.5, 0.5]; // L,R,L,R
        assert_eq!(downmix_to_mono(&stereo, 2), vec![0.5, 0.5]);
    }

    #[test]
    fn downmix_mono_is_passthrough() {
        assert_eq!(downmix_to_mono(&[0.1, 0.2], 1), vec![0.1, 0.2]);
    }

    #[test]
    fn i16_conversion_normalizes() {
        let f = i16_to_f32(&[0, -32768, 16384]);
        assert!((f[0] - 0.0).abs() < 1e-6);
        assert!((f[1] - -1.0).abs() < 1e-6);
        assert!((f[2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rms_of_silence_is_zero_and_handles_empty() {
        assert_eq!(rms(&[0.0; 8]), 0.0);
        assert_eq!(rms(&[]), 0.0);
        assert!((rms(&[1.0, -1.0, 1.0, -1.0]) - 1.0).abs() < 1e-6);
    }
}
