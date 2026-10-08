# Vendored Etterna MinaCalc

- Upstream: https://github.com/etternagame/etterna
- Commit: `2d706147cc4bcf95a8ceb24dc4f4129be207f803` (2026-09-07, Etterna 0.75.x)
- Calc version: 527 (`mina_calc_version` in `MinaCalc.cpp`)
- Licence: MIT, copied verbatim to `LICENSE`
- Decision: ADR 0022 (`docs/adr/0022-vendored-minacalc.md`)

## Copied paths

Paths are relative to the upstream root and kept as they are, because `MinaCalc.h`
includes `../Models/NoteData/NoteDataStructures.h`.

- `LICENSE` → `vendor/LICENSE`
- `src/Etterna/MinaCalc/**` (all 71 files, CMakeLists included; no file left out)
- `src/Etterna/Models/NoteData/NoteDataStructures.h`

Nothing else is needed under `STANDALONE_CALC`: the RageUtil and XML includes in `Ulbu.h`
and `phpcpp.h` in `MinaCalc.cpp` sit behind `STANDALONE_CALC`/`PHPCALC` guards.
`PatternModHelpers.h` includes `sse2neon.h` only on `__aarch64__`; that header is not
vendored, so aarch64 targets do not build.

## Local changes

Exactly one, kept in `../patches/standalone-guard.patch` and already applied here:
`MinaCalc.cpp` calls `load_calc_params_from_disk` (around line 548), which upstream only
defines outside `STANDALONE_CALC`; the patch wraps that call in the same guard. `build.rs`
never patches.

## Re-vendoring

From the repo root, with `<sha>` the new upstream commit:

```sh
tmp=$(mktemp -d)
git -C "$tmp" init -q
git -C "$tmp" remote add origin https://github.com/etternagame/etterna
git -C "$tmp" sparse-checkout set --no-cone 'src/Etterna/MinaCalc/' \
  'src/Etterna/Models/NoteData/NoteDataStructures.h' 'LICENSE'
git -C "$tmp" fetch -q --depth 1 --filter=blob:none origin <sha>
git -C "$tmp" checkout -q FETCH_HEAD

v=crates/minacalc/vendor
rm -rf "$v/src" "$v/LICENSE"
mkdir -p "$v/src/Etterna/Models/NoteData"
cp -r "$tmp/src/Etterna/MinaCalc" "$v/src/Etterna/"
cp "$tmp/src/Etterna/Models/NoteData/NoteDataStructures.h" "$v/src/Etterna/Models/NoteData/"
cp "$tmp/LICENSE" "$v/LICENSE"
patch -d "$v" -p1 < crates/minacalc/patches/standalone-guard.patch
```

Then check new `#include`s for headers outside the copied set, update the commit and calc
version above, bump `CALC_VERSION` in `src/lib.rs` to match `GetCalcVersion()`, and run
`cargo nextest run -p wolluf-minacalc` on Linux and Windows. A new calc version
recomputes every stage whose VersionKey includes it (ADR 0022).
