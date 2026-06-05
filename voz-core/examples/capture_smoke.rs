// SPDX-License-Identifier: Apache-2.0
//! Manual smoke test for dual-source capture. Run with:
//!   cargo run -p voz-core --features audio --example capture_smoke
//! Captures mic + system monitor for ~1.2s and reports sample counts/levels.

#[cfg(feature = "audio")]
fn main() {
    use std::time::Duration;
    use voz_core::capture::{default_sink, write_wav_16k_mono, Capturer};
    use voz_core::model::Source;

    println!("default sink: {:?}", default_sink());
    let cap = Capturer::start(Source::Both, "default", true).expect("failed to start capture");
    std::thread::sleep(Duration::from_millis(1200));
    let level = cap.level();
    let audio = cap.stop();
    let mic = audio.mic.as_ref().map_or(0, Vec::len);
    let sys = audio.system.as_ref().map_or(0, Vec::len);
    println!(
        "mic: {mic} samples ({:.2}s), system: {sys} samples ({:.2}s)",
        mic as f32 / 16000.0,
        sys as f32 / 16000.0
    );
    println!("levels: mic={:.4} system={:.4}", level.mic, level.system);

    if let Some(samples) = &audio.mic {
        let path = std::env::temp_dir().join("voz_smoke_mic.wav");
        write_wav_16k_mono(&path, samples).expect("wav write");
        println!("wrote {}", path.display());
    }
}

#[cfg(not(feature = "audio"))]
fn main() {
    eprintln!("run with --features audio");
}
