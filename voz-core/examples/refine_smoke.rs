// SPDX-License-Identifier: Apache-2.0
//! Manual smoke test for the default refine backend (Claude Code CLI). Run:
//!   cargo run -p voz-core --features refine --example refine_smoke

#[cfg(feature = "refine")]
fn main() {
    use voz_core::model::{RefineStyle, Speaker, Transcript, Turn};
    use voz_core::refine::{lossless_check, Refiner};
    use voz_core::refine_backends::CliRefiner;

    let t = Transcript {
        turns: vec![
            Turn {
                speaker: Speaker::Me,
                text: "so um for next week i think we should like ship the settings panel first \
                       and then uh wire up the model picker, Alex can you take the model picker"
                    .into(),
                start_ms: 0,
                end_ms: 0,
            },
            Turn {
                speaker: Speaker::Them,
                text:
                    "yeah sounds good i'll take the model picker, let's leave diarization for later"
                        .into(),
                start_ms: 0,
                end_ms: 0,
            },
        ],
        language: Some("en".into()),
    };

    let r = CliRefiner::claude_code();
    println!("refining via {} ...", r.name());
    match r.refine(&t, &RefineStyle::Adaptive) {
        Ok(note) => {
            let report = lossless_check(&t.plain_text(), &note);
            println!("--- REFINED NOTE ---\n{note}\n--- END ---");
            println!("lossless_ok={} missing={:?}", report.ok, report.missing);
        }
        Err(e) => eprintln!("refine error: {e}"),
    }
}

#[cfg(not(feature = "refine"))]
fn main() {
    eprintln!("run with --features refine");
}
