// SPDX-License-Identifier: Apache-2.0
//! Device capture (feature `audio`).
//!
//! Capture is driven by PipeWire's `pw-record`, which can target **any** node —
//! the default microphone or a sink **monitor** (the loopback of what you hear) —
//! and resample to 16 kHz mono f32 for us. Each source runs as its own subprocess
//! whose stdout we read into a buffer while tracking a live RMS level. Two streams
//! (mic = "Me", monitor = "Them") run at once for meeting capture; on stop they
//! become a [`CapturedAudio`] ready for the pipeline.
//!
//! Why `pw-record` over cpal here: cpal's ALSA backend does not reliably expose
//! PipeWire monitor sources, which are exactly what local meeting capture needs.

use crate::audio::{monitor_name_for_sink, pcm_f32le_to_samples, rms, Level};
use crate::model::Source;
use crate::pipeline::CapturedAudio;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Resolve the system default sink (its monitor is the system-audio loopback).
#[must_use]
pub fn default_sink() -> Option<String> {
    let out = Command::new("pactl")
        .arg("get-default-sink")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// A single capturing subprocess + its reader thread.
struct StreamCapture {
    child: Child,
    buf: Arc<Mutex<Vec<f32>>>,
    level: Arc<Mutex<f32>>,
    reader: Option<JoinHandle<()>>,
}

impl StreamCapture {
    /// Spawn `pw-record` for `target` (None = default source/mic), 16 kHz mono f32.
    fn spawn(target: Option<&str>) -> crate::Result<Self> {
        let mut cmd = Command::new("pw-record");
        cmd.args(["--rate", "16000", "--channels", "1", "--format", "f32"]);
        if let Some(t) = target {
            cmd.args(["--target", t]);
        }
        cmd.arg("-").stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|e| crate::Error::NoSource(format!("pw-record failed to start: {e}")))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| crate::Error::NoSource("pw-record produced no stdout".into()))?;

        let buf = Arc::new(Mutex::new(Vec::<f32>::new()));
        let level = Arc::new(Mutex::new(0.0_f32));
        let (buf_w, level_w) = (Arc::clone(&buf), Arc::clone(&level));

        let reader = std::thread::spawn(move || {
            let mut leftover: Vec<u8> = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        leftover.extend_from_slice(&chunk[..n]);
                        let usable = leftover.len() - (leftover.len() % 4);
                        if usable == 0 {
                            continue;
                        }
                        let samples = pcm_f32le_to_samples(&leftover[..usable]);
                        leftover.drain(..usable);
                        if !samples.is_empty() {
                            if let Ok(mut l) = level_w.lock() {
                                *l = rms(&samples);
                            }
                            if let Ok(mut b) = buf_w.lock() {
                                b.extend_from_slice(&samples);
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            child,
            buf,
            level,
            reader: Some(reader),
        })
    }

    fn level(&self) -> f32 {
        self.level.lock().map(|g| *g).unwrap_or(0.0)
    }

    fn stop(mut self) -> Vec<f32> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(h) = self.reader.take() {
            let _ = h.join();
        }
        self.buf.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl Drop for StreamCapture {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Captures one or both sources for the duration of a recording.
pub struct Capturer {
    mic: Option<StreamCapture>,
    system: Option<StreamCapture>,
}

impl std::fmt::Debug for Capturer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capturer")
            .field("mic", &self.mic.is_some())
            .field("system", &self.system.is_some())
            .finish()
    }
}

impl Capturer {
    /// Begin capturing per the requested `source`. The microphone uses the default
    /// device unless `mic_device` names another; the system stream uses the default
    /// sink's monitor.
    ///
    /// # Errors
    /// Returns [`crate::Error::NoSource`] if a required stream can't be started.
    pub fn start(source: Source, mic_device: &str, system_audio: bool) -> crate::Result<Self> {
        let mic = if source.has_mic() {
            let target = if mic_device == "default" {
                None
            } else {
                Some(mic_device)
            };
            Some(StreamCapture::spawn(target)?)
        } else {
            None
        };
        let system = if source.has_system() && system_audio {
            let sink = default_sink()
                .ok_or_else(|| crate::Error::NoSource("no default sink for monitor".into()))?;
            Some(StreamCapture::spawn(Some(&monitor_name_for_sink(&sink)))?)
        } else {
            None
        };
        Ok(Self { mic, system })
    }

    /// Current per-source RMS level for the live waveform.
    #[must_use]
    pub fn level(&self) -> Level {
        Level {
            mic: self.mic.as_ref().map_or(0.0, StreamCapture::level),
            system: self.system.as_ref().map_or(0.0, StreamCapture::level),
        }
    }

    /// Stop all streams and collect the captured 16 kHz mono audio.
    #[must_use]
    pub fn stop(self) -> CapturedAudio {
        CapturedAudio {
            mic: self.mic.map(StreamCapture::stop),
            system: self.system.map(StreamCapture::stop),
        }
    }
}

/// Write 16 kHz mono f32 samples to a 16-bit PCM WAV file (the "keep audio" option).
///
/// # Errors
/// Returns [`crate::Error::Storage`] on a write failure.
pub fn write_wav_16k_mono(path: &std::path::Path, samples: &[f32]) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| crate::Error::Storage(e.to_string()))?;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer
            .write_sample(v)
            .map_err(|e| crate::Error::Storage(e.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|e| crate::Error::Storage(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sink_does_not_panic() {
        // May be Some or None depending on the host; must never panic.
        let _ = default_sink();
    }

    #[test]
    fn wav_round_trips() {
        let mut path = std::env::temp_dir();
        path.push(format!("voz-cap-{}.wav", std::process::id()));
        let samples = vec![0.0_f32, 0.5, -0.5, 1.0, -1.0];
        write_wav_16k_mono(&path, &samples).unwrap();

        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.len() as usize, samples.len());
        let _ = std::fs::remove_file(&path);
    }
}
