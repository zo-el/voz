# Voz — user guide

Voz records your microphone and the meeting audio you hear, transcribes it locally,
and saves two linked notes per recording: a **Raw** verbatim transcript and a
**Refined** clean summary. Everything stays on your machine; the only optional
network use is the AI-cleanup backend and an update check.

## First run
On first launch the onboarding asks where to save notes (point it at your Obsidian
vault if you have one) and whether to use AI cleanup. A small transcription model is
fetched automatically — you can record as soon as it finishes.

## Recording
- Click the **mic** button, press the global hotkey (default `Ctrl+Super+Space`), or
  use the tray menu.
- Pick the **source**: Mic (just you), System (just the meeting), or Both.
- While recording you'll see a live waveform, a word count, and a **◉ LIVE** preview
  of the transcript. The final, authoritative transcript is produced when you stop.
- **Stop** hands the recording to the background — you're free to record again
  immediately. The note appears in **History** when it's ready.

## History
- Open any note in-panel: toggle **Raw / Refined**, **Copy**, **Open in Obsidian**,
  **Type at cursor** (dictation), **Export** (.txt/.md), **Re-refine** with a
  different style, or **Delete**.
- The search box does **full-text search** over titles *and* transcript bodies.
- **Import** an existing audio/video file to transcribe it (needs `ffmpeg` for
  non-WAV).

## Settings
Save folder · keep-audio · **Acceleration** (Auto/CPU/Vulkan/CUDA, with a "Now: …"
indicator of what's actually used) · **Model** manager (Fast/Balanced/Accurate) ·
refine **backend** · refinement **style** (Adaptive/Meeting/Memo/**Custom** prompt) ·
default source · **rebind the hotkey** · **Support** (copy diagnostics, open the log,
check for updates).

## Privacy
Audio and notes never leave your computer. With the refine backend set to **None**
(raw-only) and a model already installed, Voz makes **no** network connections.

## Stuck?
See `docs/TROUBLESHOOTING.md`.
