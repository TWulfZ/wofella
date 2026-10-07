# 0018 Label screen IPC and section audio

- Status: Accepted
- Date: 2026-10-06

## Context
F1 deliverable 4 (per-pattern precision on 200–300 gold segments, architecture §12) needs gold labels, and there are none. The pilot tried `wolluf label` and could not read patterns from its ASCII rows: they have no time-proportional spacing, no LN bodies and no column colour. Architecture §8 already names a Canvas2D Playfield as the hand-labelling tool for the gold set, so the desktop needs a Label screen that runs the same blind rounds as the CLI.

Several patterns are defined by rhythm as much as by shape (a minijack against a trill, an LN release on a beat), and a labeller hears them before reading them. The window's audio has to play in sync with the scroll.

Constraints that already hold:
- The osu! folder is read-only (D9, G6), and only the app reads it; the webview never holds a path.
- The capability surface is `core:default`, `dialog:allow-open`, `log:default` and a scoped opener (ADR 0009:55). The CSP is `default-src 'self'` with `connect-src ipc: http://ipc.localhost` and no `media-src`.
- Commands are typed by tauri-specta rc.25 and return `Result<T, IpcError>` (ADR 0009).
- Rust-side audio (`wolluf-audio`: decode, trim, stretch) is an F4 drill concern (research 03).

## Decision
- **Commands.** A `label_*` group (`label_taxonomy`, `label_sample`, `label_resolve_patterns`, `label_reshape`, `label_submit`, `label_undo`, `label_stats`) drives the rounds through `LabelingService` and writes the same `segment_label` v1 events as the CLI (ADR 0017). The `chart` group gets `chart_window(md5, fromMs, toMs, layoutId)`, the notes, timing lines and layout of one window with no segments (labelling stays blind), and `chart_audio(md5)`.
- **Audio crosses IPC as base64 in a typed DTO.** `chart_audio` returns `ChartAudioDto { mime, base64 }`. The raw-bytes route, a command returning `tauri::ipc::Response`, cannot be typed under the pinned generator. Verified in the registry sources:
  - specta `2.0.0-rc.25` `src/function/result.rs:14-30` implements `FunctionResult` only for `T: Type`, or a `Future` whose `Output: Type`, and `Result<T, E>` is a `Type` only when `T: Type` and `E: Type` (`src/type/impls.rs:141,190`);
  - tauri `2.12.0` with its `specta` feature implements `Type` for `ipc::Channel` (`src/ipc/channel.rs:193-197`) and `FunctionArg` for `State`, `AppHandle`, `Window`, `Webview` and `WebviewWindow` (`src/lib.rs:1133-1166`), but nothing for `ipc::Response` (`src/ipc/mod.rs:190`), and specta rc.25 has no `tauri` feature.

  So a `#[specta::specta]` command returning `Response` does not compile, and an untyped side command would break the generated contract. A `Vec<u8>` field would serialise as a JSON number array, about four bytes per byte; base64 costs a third.
- **The UI decodes.** The webview turns the base64 into an `ArrayBuffer` and calls WebAudio `decodeAudioData`, then loops the window with an `AudioBufferSourceNode`. Decoding a buffer fetches no URL, so the CSP and the capabilities stay as ADR 0009 left them. F1 adds no `wolluf-audio` crate.
- **Read path.** The webview sends only an md5. The app resolves it to `<Songs>/<the chart's set folder>/<AudioFilename>`: the catalog path from osu!.db, the Songs dir from the install's cfg, and `AudioFilename` from the parsed chart. `wolluf_source_osu::songs::read_song_file` rejects any chart path component that is not normal (`..`, root, drive prefix) and any `AudioFilename` that is blank, contains `/`, `\` or NUL, or is not a single normal component. It opens the file, checks size and type on the open handle, and reads at most one byte past the cap. It never writes. Containment is textual, as in `read_chart_verified`: symlinks and junctions are followed on purpose, because a Songs dir moved to another drive through a junction is a legitimate setup, and only someone who can already write into Songs can plant one.
- **Errors.** A blank `AudioFilename`, a missing file or a rejected name is `NOT_FOUND` with `error.chart_audio_unavailable`. A file above the cap is `UNSUPPORTED_FORMAT` with `error.chart_audio_too_large` and `bytes`/`maxBytes` args, since it is a wolluf limit and not a malformed request. Other IO errors are `INTERNAL`. No new `ErrorCode`.
- **Cap.** `LibraryParams::max_audio_bytes`, 64 MiB by default (D17). A full-length mp3 is a few MiB.
- **Known offset.** WebView2 may trim the LAME encoder delay of an mp3 while stable's BASS 2.4.15 may not, a difference of up to about 25 ms (research 03 l.187, l.205: BASS's handling of LAME/Xing tags is unverified). The Label screen offers a user offset slider, which is enough for hearing rhythm. Anything that judges timing against audio must measure this offset first.

## Alternatives considered
- **Tauri asset protocol scoped to the Songs dir.** `<audio>` or `fetch` would stream the file with no base64 cost. Rejected: it needs `assetProtocol.enable`, a scope covering the whole Songs tree and a CSP `media-src`/`connect-src` entry, a capability change ADR 0009 does not allow without a reason that the payload size alone does not give. It also hands the webview a filesystem path, and the scope would make every file in Songs readable from the webview instead of one checked file per md5.
- **Custom URI scheme with Range support.** `register_asynchronous_uri_scheme_protocol` could serve `wolluf-audio://<md5>` with path checks in Rust and Range requests for streaming. Rejected for F1: it adds an untyped side channel outside the generated contract, still needs a CSP `media-src` entry, and its benefit (streaming, seeking without a full load) does not matter for a 4 s looped window of a file that is a few MiB.
- **Decode in Rust and send PCM or a trimmed clip.** Would control gapless trimming and match BASS more closely. Rejected for F1: it pulls Symphonia and the F4 `wolluf-audio` crate forward, and decoded f32 PCM of a whole song is tens of MiB, larger over IPC than the encoded file.

## Consequences
- The Label screen works on the current CSP and capabilities, and the bindings stay fully typed.
- A chart's audio costs one full read and a base64 payload about 4/3 of the file per load. The UI should load it once per chart and keep it across windows of that chart.
- Files above the cap get no audio, and the screen falls back to a silent playfield.
- A future streaming need (long previews, F4 drill playback) revisits the custom scheme option. Supersede this ADR then rather than adding a second audio path beside it.
- Any feature that compares audio time with chart time must handle the encoder-delay offset above.

## Amendment 2026-10-06: pattern previews
- New command `label_pattern_examples(keymode) -> PatternExampleDto[]`, appended to the `label_*` group. Each item is a pattern id plus a `ChartWindowDto` of a synthetic chart built in `wolluf_engine::examples`, with a zero md5 and no audio. The UI draws it as a static preview on the pattern cards.
- The examples are synthetic on purpose. Real library snippets or engine segments would show the labeller what the engine thinks of real charts, which breaks the blind rule above.
- An engine test pins each example to its own pattern: the segment at the middle of its display span must have the example's id as primary, or as a secondary tag for the tag-only ids. A rule change that stops recognising an example fails CI rather than showing a misleading preview.
