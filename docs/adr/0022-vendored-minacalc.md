# 0022 Vendored Etterna MinaCalc v527 as a native calculator

- Status: Accepted
- Date: 2026-10-08

## Context
The beta skill and recommendation preview needs per-skillset chart difficulty now. MinaCalc is the only mature, MIT-licensed calculator that splits 4K rice into skillsets (Overall, Stream, Jumpstream, Handstream, Stamina, JackSpeed, Chordjack, Technical). On the pilot's library, v527 orders the REFORM 2nd 4K dans strictly (6th 22.83 … ε 34.88), its dominant skillset matches the human category on 78–94% of LeoBlack's labelled maps, and all 14 rates of a marathon take 46–95 ms. The Sunny family ranks dans better, but Sunny has no licence, and Daniel, Roxy and DanOverlay derive from it.

Architecture §3 and D8 planned `minacalc-rs` as a pure library. As of 2026-10-08 `minacalc-sys` 515.2.0 pins calc v515 (up to +2.45 Overall and −4.8 Chordjack against v527 on 4K CJ charts), runs bindgen (libclang) and `git apply` in its `build.rs`, has ubuntu-only CI and ships no LICENSE file. Upstream Etterna `2d706147cc4bcf95a8ceb24dc4f4129be207f803` (2026-09-07, Etterna 0.75.x) ships calc v527: about 13.7k lines of header-heavy C++20 with no third-party dependency, and MIT.

## Decision
- `src/Etterna/MinaCalc/**` and `NoteDataStructures.h` are vendored at that commit into a new leaf crate `wolluf-minacalc` (layer domain, no workspace dependencies). The single build-breaking line under `STANDALONE_CALC` (the params-file load, `MinaCalc.cpp:548`) is removed by a checked-in patch applied at vendoring time, never at build time.
- `build.rs` uses only `cc` (C++20, `STANDALONE_CALC`, `/EHsc` on MSVC). There is no bindgen, no git and no network at build time.
- A hand-written `extern "C"` shim catches every C++ exception. The Rust side declares the externs by hand and exposes a safe `Calc` (one per thread). This crate is the workspace's only exception to `unsafe_code = "deny"`, through a crate-level `allow`.
- `wolluf-difficulty` depends on `wolluf-minacalc` (new `layers.toml` edge) and is the only consumer.
- **D3 deviation.** MinaCalc uses platform libm (`powf`, `exp`, `log`, `erfc`) and `_mm_rsqrt_ss`, so glibc/g++ and UCRT/cl outputs can differ in the last bits. Outputs are quantised to centi-MSD `i32` before they enter any hash or golden. Stage goldens hash the adapter output (rows, rates, params), never calculator floats. Calculator values are checked by tolerance tests on both CI operating systems.
- The calc version (527) is part of every VersionKey that depends on calculator output. A new upstream version is a new vendoring commit and recomputes those stages.
- NOTICE credits Etterna MinaCalc (MIT) in the same commit that vendors it.

## Alternatives considered
- `minacalc-rs`/`minacalc-sys` from crates.io: stale calc version, libclang plus `git apply` at build time, no LICENSE file.
- MinaCalc as a subprocess, as Companella does: a second binary to ship and sign per OS, and process IO inside what should be a pure computation.
- A pure-Rust port of 13.7k lines: weeks of work, and every upstream change (four CJ changes in July 2026) would need a re-port.
- The stable star rating already in osu!.db: no IO or native code, but no skillsets. It stays the fallback if the MSVC build cannot be made to work.

## Consequences
- The workspace now builds C++20. Both CI operating systems need a C++20 compiler (g++ ≥ 11, MSVC 2022), which the runners already have.
- Calculator numbers are reproducible per platform, but not bit-identical across platforms. Anything persisted or hashed stores the quantised values.
- Architecture §3 and D8 read "vendored MinaCalc (ADR 0022)" in place of `minacalc-rs`. Calculator output is compiled in, not behind a cargo feature.
