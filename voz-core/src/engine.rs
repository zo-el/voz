// SPDX-License-Identifier: Apache-2.0
//! The orchestration runtime (feature `engine`).
//!
//! Owns the recorder and the background job queue, turns [`Command`]s into actions,
//! and emits [`Event`]s. Recording and processing are decoupled: `Stop` snapshots
//! the captured audio, spawns a worker that transcribes → **saves the raw note
//! first** → refines → saves the refined note → indexes it, while the recorder is
//! immediately free for the next take. The transcriber is injected so this module
//! builds and tests without the (slow) whisper.cpp compile.

use crate::audio::Level;
use crate::capture::{write_wav_16k_mono, CaptureTaps, Capturer};
use crate::config::{RefineCfg, Settings};
use crate::event::{Event, TrayState};
use crate::history::History;
use crate::jobs::{JobId, JobQueue, JobState};
use crate::model::{NoteMeta, RefineStyle, Source, Speaker};
use crate::pipeline::{transcribe_and_attribute, CapturedAudio};
use crate::refine::lossless_check;
use crate::refine_backends::{backend_available, build_refiner};
use crate::store::{audio_path, note_basename, raw_basename, raw_note, refined_note, save_notes};
use crate::transcribe::Transcriber;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// RFC3339 (UTC) timestamp for `created`.
#[must_use]
pub fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

/// Derive a short note title from the transcript (first words; sanitized).
#[must_use]
pub fn derive_title(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().take(6).collect();
    let title = words.join(" ");
    let title = title.trim_end_matches(['.', ',', ';', ':']).trim();
    if title.is_empty() {
        "Voice note".to_string()
    } else {
        crate::store::sanitize_filename(title)
    }
}

/// A tiny counting semaphore that bounds how many jobs do the heavy
/// transcribe/refine work *at once*. Every `Stop` still spawns a worker thread,
/// but each thread waits here for a free slot before transcribing, so a burst of
/// recordings can't launch unbounded concurrent Whisper runs. Waiters park on the
/// condvar (cheap) rather than spinning.
struct Slots {
    free: Mutex<usize>,
    cv: Condvar,
}

impl Slots {
    fn new(n: usize) -> Self {
        Slots {
            free: Mutex::new(n.max(1)),
            cv: Condvar::new(),
        }
    }

    /// Block until a slot is free, then take it. The returned guard releases the
    /// slot on drop, so it's freed on every exit path of the worker.
    fn acquire(self: &Arc<Self>) -> SlotGuard {
        let mut free = self.free.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        while *free == 0 {
            free = self
                .cv
                .wait(free)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *free -= 1;
        SlotGuard {
            slots: Arc::clone(self),
        }
    }
}

/// Releases a worker slot back to the pool when dropped.
struct SlotGuard {
    slots: Arc<Slots>,
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        let mut free = self
            .slots
            .free
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *free += 1;
        self.slots.cv.notify_one();
    }
}

/// Everything a worker needs to process one recording (no live device handles).
struct JobCtx {
    save_dir: String,
    keep_audio: bool,
    refine: RefineCfg,
    api_key: Option<String>,
    model_label: String,
    history_path: PathBuf,
    /// Spool id whose audio backs this job; cleared once the job fully succeeds so
    /// an interrupted job can be recovered on the next launch.
    spool_id: Option<String>,
    /// Worker-slot pool gating concurrent transcribe/refine work.
    slots: Arc<Slots>,
    /// Serializes the choose-unique-name → save step (closes the naming TOCTOU).
    save_gate: Arc<Mutex<()>>,
}

// ----- crash-recovery spool ---------------------------------------------------
// On Stop, the captured audio is written to a spool dir before the worker starts;
// the worker removes it only after the notes are saved. On launch, `recover()`
// re-processes any spooled audio left by a crash/quit mid-job.

fn spool_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("voz").join("spool")
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn source_tag(s: Source) -> &'static str {
    match s {
        Source::Mic => "mic",
        Source::System => "system",
        Source::Both => "both",
    }
}

fn source_from_tag(s: &str) -> Source {
    match s {
        "mic" => Source::Mic,
        "system" => Source::System,
        _ => Source::Both,
    }
}

/// Write captured audio + metadata to the spool; returns the spool id.
fn write_spool(source: Source, duration: u64, audio: &CapturedAudio) -> crate::Result<String> {
    let id = format!("{}-{}", now_nanos(), std::process::id());
    let dir = spool_dir();
    std::fs::create_dir_all(&dir)?;
    if let Some(mic) = &audio.mic {
        crate::capture::write_wav_16k_mono(&dir.join(format!("{id}.mic.wav")), mic)?;
    }
    if let Some(sys) = &audio.system {
        crate::capture::write_wav_16k_mono(&dir.join(format!("{id}.sys.wav")), sys)?;
    }
    let meta = serde_json::json!({ "source": source_tag(source), "duration": duration });
    crate::store::write_atomic(&dir.join(format!("{id}.json")), &meta.to_string())?;
    Ok(id)
}

fn clear_spool(id: &str) {
    let dir = spool_dir();
    for suffix in [".mic.wav", ".sys.wav", ".json"] {
        let _ = std::fs::remove_file(dir.join(format!("{id}{suffix}")));
    }
}

/// Load any spooled (unfinished) recordings left by a previous run.
fn recover_spool() -> Vec<(String, Source, u64, CapturedAudio)> {
    let dir = spool_dir();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()).map(String::from) else {
            continue;
        };
        let (mut source, mut duration) = (Source::Both, 0u64);
        if let Ok(txt) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                source =
                    source_from_tag(v.get("source").and_then(|s| s.as_str()).unwrap_or("both"));
                duration = v
                    .get("duration")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
            }
        }
        let mic = crate::capture::read_wav_16k_mono(&dir.join(format!("{id}.mic.wav"))).ok();
        let sys = crate::capture::read_wav_16k_mono(&dir.join(format!("{id}.sys.wav"))).ok();
        if mic.is_none() && sys.is_none() {
            clear_spool(&id);
            continue;
        }
        out.push((id, source, duration, CapturedAudio { mic, system: sys }));
    }
    out
}

fn emit(events: &Sender<Event>, ev: Event) {
    let _ = events.send(ev);
}

/// Audio (16 kHz mono) accumulated before the live preview stops updating. Past
/// this, re-transcribing the whole buffer each tick gets expensive, so the live
/// text freezes (the authoritative transcript is still produced on stop).
const PARTIAL_CAP_SAMPLES: usize = 16_000 * 150; // ~2.5 minutes

/// Background worker: every few seconds, transcribe the audio captured so far and
/// emit a non-final [`Event::Partial`]. Single-flight by construction (the loop is
/// sequential), so on a slow CPU it simply updates less often instead of piling up.
fn spawn_partials(
    taps: CaptureTaps,
    transcriber: Arc<dyn Transcriber>,
    events: Sender<Event>,
    stop: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut last_len = 0usize;
        loop {
            // ~3s between passes, but poll the stop flag often for snappy teardown.
            for _ in 0..12 {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            let audio = taps.snapshot_mixed();
            if audio.len() > PARTIAL_CAP_SAMPLES {
                continue; // preview frozen on long takes; final transcript is full
            }
            if audio.len().saturating_sub(last_len) < 16_000 {
                continue; // less than ~1s of new audio — not worth a pass
            }
            last_len = audio.len();
            if let Ok(t) = transcriber.transcribe(&audio, Speaker::Me) {
                if stop.load(Ordering::Relaxed) {
                    return; // recording ended mid-pass; drop this stale preview
                }
                let text = t.plain_text();
                if !text.trim().is_empty() {
                    emit(&events, Event::Partial { text });
                }
            }
        }
    });
}

fn tray(events: &Sender<Event>, queue: &Arc<Mutex<JobQueue>>, rec: crate::recorder::RecState) {
    let processing = queue.lock().map(|q| q.processing()).unwrap_or(0);
    emit(events, Event::Tray(TrayState::derive(rec, processing)));
}

/// Process one finished recording end-to-end. Pure of devices: takes captured
/// audio, returns nothing, emits events. Saves the raw note **before** refining
/// so a crash or refine failure never loses the transcript.
#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn process_job(
    job: JobId,
    audio: CapturedAudio,
    duration_secs: u64,
    source: Source,
    ctx: JobCtx,
    transcriber: Arc<dyn Transcriber>,
    queue: Arc<Mutex<JobQueue>>,
    events: Sender<Event>,
) {
    let set_state = |s: JobState| {
        if let Ok(mut q) = queue.lock() {
            q.set_state(job, s);
        }
        emit(&events, Event::JobState { job, state: s });
    };

    // Wait for a free worker slot before doing the heavy work, so a burst of
    // recordings can't run unbounded concurrent transcriptions. The job stays
    // `Queued` (visible in the tray) until a slot frees; the guard releases it on
    // every exit path below.
    let _slot = ctx.slots.acquire();

    // --- transcribe + attribute ---
    set_state(JobState::Transcribing);
    let transcript = match transcribe_and_attribute(&audio, transcriber.as_ref()) {
        Ok(t) => t,
        Err(e) => {
            set_state(JobState::Failed);
            emit(
                &events,
                Event::JobFailed {
                    job,
                    error: e.to_string(),
                },
            );
            return;
        }
    };
    let created = now_rfc3339();
    // Provisional name from the transcript so the RAW transcript can be persisted
    // *before* refine runs (the source-of-truth guarantee). The final, readable
    // name depends on the refined title and is resolved once refine completes.
    let prov_title = derive_title(&transcript.plain_text());
    let prov_base = note_basename(&created, "Note", &prov_title);
    let prov_raw_path = crate::store::expand_tilde(&ctx.save_dir)
        .join("raw")
        .join(format!("{}.md", raw_basename(&prov_base)));

    // --- persist RAW first (source of truth) ---
    // NOTE: the raw note is deliberately written twice — here under the provisional
    // name (so the transcript is on disk *before* refine can crash), and again by
    // `save_notes` below under the final readable name. Don't "optimize" this into a
    // single post-refine write: that would reintroduce the window where a refine
    // crash loses the transcript. The provisional copy is dropped after the final
    // save succeeds (or kept on failure, where it's the surviving copy).
    if let Err(e) = crate::store::write_atomic(
        &prov_raw_path,
        &raw_note(&created, &transcript, &prov_base),
    ) {
        set_state(JobState::Failed);
        emit(
            &events,
            Event::JobFailed {
                job,
                error: e.to_string(),
            },
        );
        return;
    }
    if let Ok(mut q) = queue.lock() {
        q.mark_raw_saved(job);
    }
    emit(
        &events,
        Event::RawTranscript {
            job,
            text: transcript.clone(),
        },
    );

    // --- refine (optional; unavailable backend or failure keeps the raw note) ---
    let refiner = if backend_available(&ctx.refine, ctx.api_key.is_some()) {
        build_refiner(&ctx.refine, ctx.api_key.clone())
    } else {
        None // CLI not installed / no key -> raw-only, no scary error
    };
    let backend_name = refiner
        .as_ref()
        .map_or("None".to_string(), |r| r.name().to_string());
    let mut refined_body = String::new();
    let mut lossless_ok = true;
    if let Some(r) = refiner {
        set_state(JobState::Refining);
        match r.refine(&transcript, &ctx.refine.style) {
            Ok(body) => {
                lossless_ok = lossless_check(&transcript.plain_text(), &body).ok;
                refined_body = body;
            }
            Err(e) => {
                lossless_ok = false;
                emit(
                    &events,
                    Event::JobFailed {
                        job,
                        error: format!("refine: {e}"),
                    },
                );
                // continue: we still save a refined note that falls back to raw
            }
        }
    }

    // Lift the refiner's `Title:` line + resolve the kind; fall back to the
    // transcript-derived title when refine produced nothing usable.
    let refined = crate::refine::interpret_refined(&refined_body, &ctx.refine.style, &prov_title);

    // Hold the save gate across choose-name → write so a concurrent job can't pick
    // the same name between our existence check and write (the unique_basename TOCTOU).
    let _save_guard = ctx
        .save_gate
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    // Resolve the final, readable, collision-free name now that the title is known.
    let base = crate::store::unique_basename(
        &ctx.save_dir,
        &note_basename(&created, &refined.kind, &refined.title),
    );
    let raw_base = raw_basename(&base);

    // keep audio (best-effort; never fails the job) under the final name.
    if ctx.keep_audio {
        if let Some(mic) = &audio.mic {
            let _ = write_wav_16k_mono(&audio_path(&ctx.save_dir, &base), mic);
        } else if let Some(sys) = &audio.system {
            let _ = write_wav_16k_mono(&audio_path(&ctx.save_dir, &base), sys);
        }
    }

    let body_for_note = if refined.present {
        refined.body.clone()
    } else {
        transcript.plain_text()
    };
    let raw_md = raw_note(&created, &transcript, &base);
    let meta = NoteMeta {
        created,
        duration_secs,
        source,
        voices: transcript.voices().into_iter().map(String::from).collect(),
        model: ctx.model_label.clone(),
        refine_backend: backend_name,
        lossless_ok,
        words: transcript.word_count(),
        title: refined.title.clone(),
        kind: refined.kind.clone(),
    };
    let refined_md = refined_note(&meta, &body_for_note, &raw_base);

    let saved = match save_notes(&ctx.save_dir, &base, &raw_base, &refined_md, &raw_md) {
        Ok(p) => p,
        Err(e) => {
            set_state(JobState::Failed);
            // The transcript isn't lost: the spool is left in place, so the job
            // re-runs from the spooled audio on next launch. With a spool present,
            // the provisional raw is now redundant — drop it so a failed save
            // doesn't leave an orphan (with a dangling refined link). Without a
            // spool, keep it: it's the only persisted copy of the transcript.
            if ctx.spool_id.is_some() {
                let _ = std::fs::remove_file(&prov_raw_path);
            }
            emit(
                &events,
                Event::JobFailed {
                    job,
                    error: e.to_string(),
                },
            );
            return;
        }
    };
    // Finalization may have moved the raw note to a readable name — drop the
    // provisional one written before refine (the transcript is now safe at `saved.raw`).
    if saved.raw != prov_raw_path {
        let _ = std::fs::remove_file(&prov_raw_path);
    }

    // index in history (best-effort); store the raw transcript for content search
    if let Ok(h) = History::open(&ctx.history_path) {
        let _ = h.insert(
            &refined.title,
            &meta,
            saved.refined.to_str().unwrap_or_default(),
            saved.raw.to_str().unwrap_or_default(),
            &transcript.plain_text(),
        );
    }

    // job fully succeeded — drop its recovery spool.
    if let Some(id) = &ctx.spool_id {
        clear_spool(id);
    }

    emit(
        &events,
        Event::RefineDone {
            job,
            refined: body_for_note,
            lossless_ok,
        },
    );
    emit(
        &events,
        Event::Saved {
            job,
            refined: saved.refined,
            raw: saved.raw,
        },
    );
    set_state(JobState::Done);
    emit(
        &events,
        Event::Notify {
            title: format!("Note ready: {}", refined.title),
            body: if lossless_ok {
                "Saved".into()
            } else {
                "Saved (review: detail may differ)".into()
            },
            job,
        },
    );
    tray(&events, &queue, crate::recorder::RecState::Idle);
}

/// The engine: drive it with [`Engine::handle`]-style methods; consume [`Event`]s
/// from the receiver paired with the `events` sender you pass to [`Engine::new`].
pub struct Engine {
    settings: Settings,
    recorder: crate::recorder::Recorder,
    queue: Arc<Mutex<JobQueue>>,
    capturer: Option<Capturer>,
    /// Signal that stops the live-partials worker for the current recording.
    partials_stop: Option<Arc<AtomicBool>>,
    /// Start of the current *active* (un-paused) recording span; `None` while paused.
    started: Option<Instant>,
    /// Active recording time accumulated across earlier spans of this take (so the
    /// stored duration excludes paused stretches).
    recorded: Duration,
    started_source: Source,
    transcriber: Arc<dyn Transcriber>,
    history_path: PathBuf,
    api_key: Option<String>,
    events: Sender<Event>,
    /// Bounds concurrent transcribe/refine work across all jobs.
    slots: Arc<Slots>,
    /// Serializes the choose-unique-name → save step so two concurrent jobs can't
    /// both claim the same filename (closes the `unique_basename` TOCTOU).
    save_gate: Arc<Mutex<()>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("state", &self.recorder.state())
            .finish()
    }
}

impl Engine {
    /// Create an engine. `transcriber` is injected (the app supplies a
    /// `WhisperTranscriber`); `history_path` defaults via [`History::default_path`].
    #[must_use]
    pub fn new(
        settings: Settings,
        transcriber: Arc<dyn Transcriber>,
        history_path: PathBuf,
        events: Sender<Event>,
    ) -> Self {
        let concurrency = 2;
        Engine {
            settings,
            recorder: crate::recorder::Recorder::default(),
            queue: Arc::new(Mutex::new(JobQueue::new())),
            slots: Arc::new(Slots::new(concurrency)),
            save_gate: Arc::new(Mutex::new(())),
            capturer: None,
            partials_stop: None,
            started: None,
            recorded: Duration::ZERO,
            started_source: Source::Both,
            transcriber,
            history_path,
            api_key: None,
            events,
        }
    }

    /// Provide the Claude API key (read from the OS secret service by the app).
    pub fn set_api_key(&mut self, key: Option<String>) {
        self.api_key = key;
    }

    #[must_use]
    pub fn state(&self) -> crate::recorder::RecState {
        self.recorder.state()
    }

    /// Current capture levels (zero when idle).
    #[must_use]
    pub fn level(&self) -> Level {
        self.capturer
            .as_ref()
            .map_or(Level::default(), Capturer::level)
    }

    /// Snapshot of the job queue for the History tab.
    #[must_use]
    pub fn jobs(&self) -> Vec<crate::jobs::Job> {
        self.queue
            .lock()
            .map(|q| q.jobs().to_vec())
            .unwrap_or_default()
    }

    /// Start recording from `source`.
    ///
    /// # Errors
    /// Fails on an invalid transition or if capture can't start.
    pub fn start(&mut self, source: Source) -> crate::Result<()> {
        self.recorder.start()?;
        let cap = Capturer::start(
            source,
            &self.settings.sources.mic_device,
            self.settings.sources.system_audio,
        )?;
        // Live transcription preview: a worker taps the capture buffers and emits
        // `Partial` events while recording. The buffers are shared (Arc), so this
        // never blocks capture or needs the engine lock.
        let stop = Arc::new(AtomicBool::new(false));
        spawn_partials(
            cap.taps(),
            Arc::clone(&self.transcriber),
            self.events.clone(),
            Arc::clone(&stop),
        );
        self.partials_stop = Some(stop);
        self.capturer = Some(cap);
        self.started = Some(Instant::now());
        self.recorded = Duration::ZERO;
        self.started_source = source;
        emit(&self.events, Event::RecState(self.recorder.state()));
        tray(&self.events, &self.queue, self.recorder.state());
        Ok(())
    }

    /// Active recording time so far, excluding paused stretches.
    fn active_elapsed(&self) -> Duration {
        self.recorded + self.started.map_or(Duration::ZERO, |s| s.elapsed())
    }

    /// # Errors
    /// Fails on an invalid transition.
    pub fn pause(&mut self) -> crate::Result<()> {
        self.recorder.pause()?;
        // Bank the active span and actually stop capturing audio (drop the stream).
        if let Some(s) = self.started.take() {
            self.recorded += s.elapsed();
        }
        if let Some(c) = &self.capturer {
            c.pause();
        }
        emit(&self.events, Event::RecState(self.recorder.state()));
        Ok(())
    }

    /// # Errors
    /// Fails on an invalid transition.
    pub fn resume(&mut self) -> crate::Result<()> {
        self.recorder.resume()?;
        self.started = Some(Instant::now());
        if let Some(c) = &self.capturer {
            c.resume();
        }
        emit(&self.events, Event::RecState(self.recorder.state()));
        Ok(())
    }

    /// Discard the current recording without producing a note.
    ///
    /// # Errors
    /// Fails if not recording.
    pub fn cancel(&mut self) -> crate::Result<()> {
        self.recorder.cancel()?;
        self.stop_partials();
        self.capturer.take(); // dropping stops the streams
        self.started = None;
        self.recorded = Duration::ZERO;
        emit(&self.events, Event::RecState(self.recorder.state()));
        tray(&self.events, &self.queue, self.recorder.state());
        Ok(())
    }

    /// Stop recording and enqueue a background job; the recorder returns to Idle.
    /// The captured audio is spooled to disk first, so a crash mid-processing can
    /// be recovered on the next launch (see [`Engine::recover`]).
    ///
    /// # Errors
    /// Fails if not recording.
    pub fn stop(&mut self) -> crate::Result<()> {
        self.recorder.stop()?;
        self.stop_partials();
        let duration = self.active_elapsed().as_secs();
        self.started = None;
        self.recorded = Duration::ZERO;
        let audio = self.capturer.take().map(Capturer::stop).unwrap_or_default();
        let source = self.started_source;
        emit(&self.events, Event::RecState(self.recorder.state()));

        let spool_id = write_spool(source, duration, &audio).ok();
        self.spawn_processing(source, duration, audio, spool_id);
        Ok(())
    }

    /// Re-process any recordings spooled by a previous run that crashed/quit
    /// mid-job. Call once at startup. Returns how many jobs were recovered.
    pub fn recover(&mut self) -> usize {
        let pending = recover_spool();
        let n = pending.len();
        for (id, source, duration, audio) in pending {
            self.spawn_processing(source, duration, audio, Some(id));
        }
        n
    }

    /// Import an existing audio/video file: decode it and run the same
    /// transcribe → refine → save pipeline as a recording. Doesn't touch the
    /// recorder, so it works even while recording. Treated as a single ("Me")
    /// source since imported files aren't dual-stream.
    ///
    /// # Errors
    /// Fails if the file can't be decoded or contains no audio.
    pub fn import_file(&self, path: &std::path::Path) -> crate::Result<()> {
        self.import_samples(crate::capture::decode_to_16k_mono(path)?)
    }

    /// Enqueue already-decoded 16 kHz mono audio (lets the app decode off the engine
    /// lock, then call this for the quick enqueue).
    ///
    /// # Errors
    /// Fails if `samples` is empty.
    pub fn import_samples(&self, samples: Vec<f32>) -> crate::Result<()> {
        if samples.is_empty() {
            return Err(crate::Error::NoSource("no audio in file".into()));
        }
        let duration = (samples.len() / 16_000) as u64;
        let audio = CapturedAudio {
            mic: Some(samples),
            system: None,
        };
        let spool_id = write_spool(Source::Mic, duration, &audio).ok();
        self.spawn_processing(Source::Mic, duration, audio, spool_id);
        Ok(())
    }

    /// Enqueue a job for already-captured audio and spawn its worker.
    fn spawn_processing(
        &self,
        source: Source,
        duration: u64,
        audio: CapturedAudio,
        spool_id: Option<String>,
    ) {
        let job = self
            .queue
            .lock()
            .map(|mut q| q.enqueue(source))
            .unwrap_or(JobId(0));
        let ctx = JobCtx {
            save_dir: self.settings.general.save_dir.clone(),
            keep_audio: self.settings.general.keep_audio,
            refine: self.settings.refine.clone(),
            api_key: self.api_key.clone(),
            model_label: format!("whisper {}", self.settings.transcription.model),
            history_path: self.history_path.clone(),
            spool_id,
            slots: Arc::clone(&self.slots),
            save_gate: Arc::clone(&self.save_gate),
        };
        let (tx, q, t) = (
            self.events.clone(),
            Arc::clone(&self.queue),
            Arc::clone(&self.transcriber),
        );
        tray(&self.events, &self.queue, self.recorder.state());
        std::thread::spawn(move || {
            process_job(job, audio, duration, source, ctx, t, q, tx);
        });
    }

    /// Signal the live-partials worker (if any) to exit.
    fn stop_partials(&mut self) {
        if let Some(flag) = self.partials_stop.take() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Replace settings (e.g. after the user edits them).
    pub fn update_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// Swap the transcriber (e.g. after a model finishes downloading at first run).
    pub fn set_transcriber(&mut self, transcriber: Arc<dyn Transcriber>) {
        self.transcriber = transcriber;
    }

    /// Style currently configured (for re-refine).
    #[must_use]
    pub fn refine_style(&self) -> RefineStyle {
        self.settings.refine.style.clone()
    }

    /// Read the current settings (for the Settings view).
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RefineBackend;
    use crate::model::{Speaker, Transcript, Turn};
    use std::sync::mpsc::channel;

    struct MockTranscriber;
    impl Transcriber for MockTranscriber {
        fn transcribe(&self, _audio: &[f32], speaker: Speaker) -> crate::Result<Transcript> {
            Ok(Transcript {
                turns: vec![Turn {
                    speaker,
                    text: "Planning sync ship the settings panel first".into(),
                    start_ms: 0,
                    end_ms: 0,
                }],
                language: Some("en".into()),
            })
        }
    }

    #[test]
    fn derive_title_takes_first_words() {
        assert_eq!(
            derive_title("hello there my friend, how are you"),
            "hello there my friend, how are"
        );
        assert_eq!(derive_title("   "), "Voice note");
    }

    #[test]
    fn slots_bound_concurrency() {
        // One slot: a second acquirer must block until the first is released.
        let slots = Arc::new(Slots::new(1));
        let held = slots.acquire();
        let (tx, rx) = channel();
        let s2 = Arc::clone(&slots);
        let waiter = std::thread::spawn(move || {
            let _g = s2.acquire(); // blocks while `held` is alive
            let _ = tx.send(());
        });
        // While the only slot is held, the waiter cannot have proceeded.
        assert!(rx.try_recv().is_err());
        drop(held); // free the slot
        rx.recv_timeout(std::time::Duration::from_secs(2))
            .expect("waiter should acquire the slot once it's freed");
        waiter.join().unwrap();
    }

    #[test]
    fn process_job_saves_raw_first_then_refined_and_indexes() {
        let dir = std::env::temp_dir().join(format!("voz-engine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (tx, rx) = channel();
        let queue = Arc::new(Mutex::new(JobQueue::new()));
        let job = queue.lock().unwrap().enqueue(Source::Mic);
        let ctx = JobCtx {
            save_dir: dir.to_str().unwrap().to_string(),
            keep_audio: false,
            refine: RefineCfg {
                backend: RefineBackend::None, // offline: raw-only, no external deps
                ollama_model: String::new(),
                style: RefineStyle::Adaptive,
                lossless_guard: true,
            },
            api_key: None,
            model_label: "mock".into(),
            history_path: dir.join("history.sqlite"),
            spool_id: None,
            slots: Arc::new(Slots::new(1)),
            save_gate: Arc::new(Mutex::new(())),
        };
        let audio = CapturedAudio {
            mic: Some(vec![0.0; 4]),
            system: None,
        };

        process_job(
            job,
            audio,
            5,
            Source::Mic,
            ctx,
            Arc::new(MockTranscriber),
            Arc::clone(&queue),
            tx,
        );

        // raw note saved before any refined note; both exist
        let raw = dir.join("raw");
        assert!(raw.is_dir());
        let refined: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
            .collect();
        assert_eq!(refined.len(), 1);
        // events: ends with Done + a notification
        let events: Vec<Event> = rx.try_iter().collect();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::RawTranscript { .. })));
        assert!(events.iter().any(|e| matches!(e, Event::Saved { .. })));
        assert!(events.iter().any(|e| matches!(
            e,
            Event::JobState {
                state: JobState::Done,
                ..
            }
        )));
        assert!(events.iter().any(|e| matches!(e, Event::Notify { .. })));
        // history indexed
        let h = History::open(&dir.join("history.sqlite")).unwrap();
        assert_eq!(h.recent(10).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn spool_round_trips_and_clears() {
        // Use an isolated HOME so the spool dir doesn't collide with a real one.
        let home = std::env::temp_dir().join(format!("voz-spool-home-{}", now_nanos()));
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("XDG_DATA_HOME", &home);

        let audio = CapturedAudio {
            mic: Some(vec![0.25; 1600]),
            system: Some(vec![-0.25; 1600]),
        };
        let id = write_spool(Source::Both, 7, &audio).unwrap();

        let recovered = recover_spool();
        assert_eq!(recovered.len(), 1);
        let (rid, src, dur, ra) = &recovered[0];
        assert_eq!(rid, &id);
        assert_eq!(*src, Source::Both);
        assert_eq!(*dur, 7);
        assert_eq!(ra.mic.as_ref().unwrap().len(), 1600);
        assert!(ra.system.is_some());

        clear_spool(&id);
        assert!(recover_spool().is_empty());
        let _ = std::fs::remove_dir_all(&home);
        std::env::remove_var("XDG_DATA_HOME");
    }
}
