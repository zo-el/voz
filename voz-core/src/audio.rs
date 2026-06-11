// SPDX-License-Identifier: Apache-2.0
//! Audio types and pure-DSP helpers (no device I/O here).
//!
//! Device capture lives in [`crate::capture`] behind the `audio` feature; it
//! drives PipeWire's `pw-record` (which targets any node — mic or a sink
//! monitor — and resamples to 16 kHz mono f32). The pure helpers below are used
//! by that layer and are fully tested with no devices.

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

/// Parse a little-endian 32-bit float PCM byte stream into samples. Any trailing
/// partial frame (length not a multiple of 4) is ignored.
#[must_use]
pub fn pcm_f32le_to_samples(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

/// The PipeWire/PulseAudio monitor source name for a given sink (the loopback of
/// what's playing on it) — e.g. `alsa_output.pci-0000_00_1f.3.analog-stereo` →
/// `...analog-stereo.monitor`.
#[must_use]
pub fn monitor_name_for_sink(sink: &str) -> String {
    if sink.ends_with(".monitor") {
        sink.to_string()
    } else {
        format!("{sink}.monitor")
    }
}

/// Resample mono PCM from `in_rate` to `out_rate`. Integer downsample ratios use
/// box-average decimation (a cheap anti-alias low-pass); other ratios use linear
/// interpolation. Capture normally asks `pw-record` for 16 kHz directly, so this
/// is a fallback for native-rate buffers.
#[must_use]
pub fn resample_mono(input: &[f32], in_rate: u32, out_rate: u32) -> Vec<f32> {
    // `in_rate == 0` guards a malformed/hostile WAV header: without it the
    // integer-ratio branch below computes `chunks(in_rate / out_rate)` = `chunks(0)`,
    // which panics.
    if in_rate == out_rate || input.is_empty() || in_rate == 0 || out_rate == 0 {
        return input.to_vec();
    }
    if in_rate % out_rate == 0 {
        let factor = (in_rate / out_rate) as usize;
        return input
            .chunks(factor)
            .map(|c| c.iter().sum::<f32>() / c.len() as f32)
            .collect();
    }
    let out_len = (input.len() as u64 * u64::from(out_rate) / u64::from(in_rate)) as usize;
    let step = input.len() as f32 / out_len.max(1) as f32;
    (0..out_len)
        .map(|i| {
            let pos = i as f32 * step;
            let idx = pos.floor() as usize;
            let frac = pos - pos.floor();
            let a = input[idx.min(input.len() - 1)];
            let b = input[(idx + 1).min(input.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
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

    #[test]
    fn pcm_parsing_handles_partial_frames() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1.0f32.to_le_bytes());
        bytes.extend_from_slice(&(-0.5f32).to_le_bytes());
        bytes.push(0x00); // trailing partial frame, ignored
        let s = pcm_f32le_to_samples(&bytes);
        assert_eq!(s.len(), 2);
        assert!((s[0] - 1.0).abs() < 1e-6 && (s[1] + 0.5).abs() < 1e-6);
    }

    #[test]
    fn monitor_name_derivation() {
        assert_eq!(
            monitor_name_for_sink("sink.analog-stereo"),
            "sink.analog-stereo.monitor"
        );
        assert_eq!(monitor_name_for_sink("x.monitor"), "x.monitor");
    }

    #[test]
    fn resample_integer_ratio_decimates() {
        // 48k -> 16k is factor 3: each output is the mean of 3 inputs.
        let out = resample_mono(&[3.0, 3.0, 3.0, 9.0, 9.0, 9.0], 48_000, 16_000);
        assert_eq!(out, vec![3.0, 9.0]);
    }

    #[test]
    fn resample_passthrough_and_empty() {
        assert_eq!(resample_mono(&[0.1, 0.2], 16_000, 16_000), vec![0.1, 0.2]);
        assert_eq!(resample_mono(&[], 48_000, 16_000), Vec::<f32>::new());
    }

    #[test]
    fn resample_zero_rate_does_not_panic() {
        // A malformed WAV reporting sample_rate 0 must not panic (chunks(0)).
        assert_eq!(resample_mono(&[0.1, 0.2], 0, 16_000), vec![0.1, 0.2]);
        assert_eq!(resample_mono(&[0.1, 0.2], 16_000, 0), vec![0.1, 0.2]);
    }

    #[test]
    fn resample_fractional_changes_length() {
        let input = vec![0.0f32; 44_100];
        let out = resample_mono(&input, 44_100, 16_000);
        assert_eq!(out.len(), 16_000);
    }
}
