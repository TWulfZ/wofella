# Vendored Signalsmith Stretch

- Decision: ADR 0025 (`docs/adr/0025-rate-copies.md`); build rules from ADR 0022.
- Licence: MIT for both projects, each copied verbatim next to its headers (`LICENSE.txt`).
- No local changes. `build.rs` never patches, fetches or generates anything.

## Signalsmith Stretch

- Upstream: https://github.com/Signalsmith-Audio/signalsmith-stretch
- Tag `1.4.0`, commit `a670068d9aeb64913331d5cc29337b19a457a7df` (2026-09-25).
  The header's own `version` constant still reads `{1, 3, 2}`; the tag is authoritative.
- Copyright (c) 2022 Geraint Luff / Signalsmith Audio Ltd.
- Files: `signalsmith-stretch.h`, `LICENSE.txt` → `signalsmith-stretch/`.
  The upstream `include/signalsmith-stretch/signalsmith-stretch.h` is a one-line forwarder
  to the root header and is not needed.

## Signalsmith Linear (dependency of Stretch)

- Upstream: https://github.com/Signalsmith-Audio/linear
- Tag `0.6.4`, commit `de55e6a50ffcf6f8f43f649692d94691c7025151` (2026-09-25): the
  version Stretch 1.4.0 pins in its `CMakeLists.txt`.
- Copyright (c) 2025 Signalsmith Audio
- Files → `signalsmith-linear/`: `fft.h`, `stft.h`, `LICENSE.txt`, and the forwarders
  `include/signalsmith-linear/fft.h`, `include/signalsmith-linear/stft.h`.

Stretch includes `"signalsmith-linear/stft.h"`, which needs only `fft.h`. The other Linear
headers (`linear.h`, `approx.h`, `platform/*`) are reached only through the
`SIGNALSMITH_USE_ACCELERATE/IPP/PFFFT/CMSISDSP/XSIMD` defines, which the build never sets,
so the portable FFT is used on every platform.

## Re-vendoring

From the repo root, with `<stretch-tag>` the new Stretch release and `<linear-tag>` the
Linear tag its `CMakeLists.txt` pins (`GIT_TAG`):

```sh
tmp=$(mktemp -d)
git clone -q --depth 1 --branch <stretch-tag> https://github.com/Signalsmith-Audio/signalsmith-stretch "$tmp/stretch"
git clone -q --depth 1 --branch <linear-tag> https://github.com/Signalsmith-Audio/linear "$tmp/linear"
grep -n GIT_TAG "$tmp/stretch/CMakeLists.txt"

v=crates/signalsmith/vendor
rm -rf "$v/signalsmith-stretch" "$v/signalsmith-linear"
mkdir -p "$v/signalsmith-stretch" "$v/signalsmith-linear/include/signalsmith-linear"
cp "$tmp/stretch/signalsmith-stretch.h" "$tmp/stretch/LICENSE.txt" "$v/signalsmith-stretch/"
cp "$tmp/linear/fft.h" "$tmp/linear/stft.h" "$tmp/linear/LICENSE.txt" "$v/signalsmith-linear/"
cp "$tmp/linear/include/signalsmith-linear/fft.h" "$tmp/linear/include/signalsmith-linear/stft.h" \
  "$v/signalsmith-linear/include/signalsmith-linear/"
```

Then check every `#include "..."` in the copied headers resolves inside this folder, update
the tags, commits and copyright lines above and in NOTICE, and run
`cargo nextest run -p wolluf-signalsmith -p wolluf-audio` on Linux and Windows. The click
track tests (onset drift ≤ 2 ms, length ± 10 ms) are the acceptance gate; outputs are
never compared by hash.
