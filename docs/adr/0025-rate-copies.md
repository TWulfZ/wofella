# 0025 Rate copies: the first F4 export, pulled forward

- Status: Accepted
- Date: 2026-10-08

## Context
Recommendations (ADR 0024) suggest charts at rates such as 1.15x, which stable cannot play without a rate-edited copy. The user asked wofella to generate those copies itself (2026-10-08). A rate copy is the simplest rendered drill of architecture §1 (`t' = t / r`), so it is the first slice of F4 (§12), pulled forward.

Existing tools were read at the source (research in this branch's history, rate-generation track):
- Companella runs a bundled GPL ffmpeg build as a subprocess and rewrites hit-sample sets of every plain note as if they were LN tails.
- osu-trainer has no licence and uses SoundTouch and LAME (LGPL).
- Both write the new `.osu` and audio into the source set folder. osu-trainer truncates times with `int(t / r)` and keeps BeatmapSetID. The pilot's library already holds 533 such copies.

ADR 0008 keeps F4 audio permissive and in-process.

## Decision
- **`wolluf-drills` (domain, pure)** rewrites `.osu` bytes line by line. It does not round-trip through the lossy chart model.
  - **Times:** every time becomes `round_half_up(t / r)`, always from the original value. This covers HitObject heads and LN tails (only when type bit 128 is set; hit-sample fields stay byte-identical), Break periods, Bookmarks, storyboard `Sample` events and `PreviewTime`; `PreviewTime -1` stays `-1`.
  - **Timing points:** red-line offsets keep 3 decimals and beatLength is divided by `r`. Green lines scale only their offset.
  - **Unchanged:** `AudioLeadIn`, OD and HP (DT parity).
  - **Version and tags:** `Version` gets ` <r>x (<bpm>bpm)`, and `Tags` gets `wofella`.
  - **Ids:** `BeatmapID:0`, `BeatmapSetID` kept.
  - **Audio:** `AudioFilename` becomes `<base> <r>x.ogg`.
- **`wolluf-signalsmith` (domain leaf)** vendors Signalsmith Stretch (MIT, header-only C++) behind a hand-written C ABI built with `cc`, the same pattern as `wolluf-minacalc` (ADR 0022). It is the second crate allowed `unsafe`. The crates.io wrapper is rejected because its build runs bindgen and needs libclang.
- **`wolluf-audio` (adapter)** decodes with Symphonia (MPL-2.0), time-stretches with pitch kept (the DT default; the pilot keeps pitch on 76% of its copies), and encodes Ogg Vorbis at quality ≈ 0.5 with `vorbis_rs` (BSD-3). There is no mp3 encoder, no ffmpeg and no SoundTouch. New edge: `wolluf-audio → wolluf-signalsmith`. `wolluf-engine → wolluf-drills` and `wolluf-app → wolluf-audio` already exist in `xtask/layers.toml`.
- **Writing into osu!** goes only through `app::export`, behind an `ExportPermit` minted by `confirm(preview_id)` after the user confirms a recorded preview (D9).
  - Files land in the source set folder and never overwrite. Each file is written to a temp name, synced, then published with a hard link (never replaces; a reserve-then-rename fallback for volumes without hard links). A copy is all-or-nothing: if the `.osu` cannot be published, the audio this run published is removed.
  - An existing audio file at that rate is reused only if it is a non-empty Ogg Vorbis file; an existing `.osu` of the same name is skipped only if it names that audio, otherwise the plan refuses with `already_exists`.
  - Names are refused (`unsafe_name`) when they hold path separators, `:<>"|?*`, exceed 255 bytes, or make a path over 259 characters (stable is a .NET Framework app).
  - No collection.db write, so the osu!-not-running check and the backup are not needed.
  - The user presses F5 in song select. Copies are found again by the `wofella` tag, so no manifest table is needed.
- Keysounded maps (`Sample` events in the `.osu` or any `.osb` of the set, after storyboard `[Variables]` expansion; custom hit-sample files) and LN-heavy maps are refused for now.

## Alternatives considered
- ffmpeg sidecar: GPL redistribution and source-offer duties (research 03 l.193).
- SoundTouch / Rubber Band in-process: LGPL / GPL, excluded by ADR 0008.
- An `.osz` opened through the shell: unclear whether stable merges a partial set into an existing folder (open question); direct writes are what the pilot's existing copies already rely on.
- Pitch-follows-rate (NC) only: simpler (resampling), but not what DT players train with. It can come later as an option.

## Consequences
- Two native C++ leaf crates now exist; both follow the ADR 0022 build rules (cc only, exceptions caught, quantised or tolerance-tested outputs).
- Audio output differs across platforms in the last bits; tests compare onsets and lengths with tolerances, never hashes.
- Architecture §3 lists `wolluf-signalsmith`; §12 F4 starts with rate copies.
