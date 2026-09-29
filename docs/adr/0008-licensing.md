# 0008 Licensing: project licence, clean-room Sunny, MIT ports and the dependency allowlist

- Status: Accepted
- Date: 2026-09-28

## Context
wolluf is a desktop binary that will be distributed. The prior art it learns from has mixed licences (research `01-landscape-verified.txt` l.63, l.76, l.106, l.119; `03-maniahub-rejudge-drills-sessions-audit.txt` l.48, l.171; `00-plan-es.md` l.75):

| Source | Licence | What it means for wolluf |
|---|---|---|
| YAVSRG `prelude/` (Interlude patterns, judge ruleset) | MIT | Portable with attribution |
| YAVSRG `interlude/`, `online/` | GPL-3.0 | Not portable. Only `prelude/` may be ported |
| mania-hub `algorithms/` | MIT (© 2026 aleju03) | Portable with attribution. The rest of the repo has no licence: ideas only |
| LeoBlack interval tables and analysers | MIT | Portable with attribution |
| sunnyxxy Star-Rating-Rebirth (Sunny) | none (no licence file) | All rights reserved. Code must not be copied |
| SPMRating, Dan-Overlay | MIT, but they build on or vendor the unlicensed Sunny code | The Sunny risk applies to them transitively (research 01 l.49; research `02-7k-bms-stable-verified.txt` l.54) |
| tosu / gosumemory | LGPL-3.0 / GPL-3.0 | Must not be linked in-process |
| slider (Python parser) | LGPL-3.0 | Not used |
| Quaver.API | MPL-2.0 (file-level copyleft) | Reference only, unless used unmodified as a dependency |

The workspace also pulls hundreds of transitive crates, and one GPL crate slipping in would force the whole binary under the GPL. The project needs its own licence before the first commit that carries code (spec 001 Q1).

## Decision
- **Project licence: MIT**, `Copyright (c) 2026 TWulfZ` (user decision 2026-09-28). `LICENSE` holds the MIT text, `NOTICE` repeats the copyright line, and `[workspace.package] license = "MIT"` is inherited by every member. `publish = false` stays until the first release.
- **No GPL, LGPL or AGPL code in-process (D16).** This covers both dependencies and ported code.
- **tosu runs out of process.** wolluf talks to it over its websocket as an optional separate program (F5), which keeps the LGPL/GPL boundary at the process line. Live memory reading is a non-goal (§1).
- **Sunny is reimplemented clean-room from its paper**, never from the repository code. MIT ports of Sunny (zzzzv's C# port, Metron) derive from unlicensed code themselves. They may serve only as black-box numerical oracles in tests, never as source to port.
- **MIT ports** (Interlude `prelude/`, mania-hub `algorithms/`, LeoBlack) are attributed in `NOTICE` **in the same commit that brings the port in**. Each entry names the project, the upstream path, the licence, the copyright line and where the port lives. Unlicensed repositories (the rest of mania-hub, Star-Rating-Rebirth, unlicensed generators) are ideas only.
- **Dependency allowlist** (`deny.toml`, `cargo deny check` in CI). Exactly: `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `MPL-2.0`, `Zlib`, `ISC`, `Unicode-3.0`, `CC0-1.0`. Anything else fails, and GPL/LGPL/AGPL can never be added. The xtask test `deny_config::tests::allowlist_is_exact` pins the set. Two entries go beyond architecture §3's list, with reasons:
  - `Unicode-3.0`: required by `unicode-ident` (`(MIT OR Apache-2.0) AND Unicode-3.0`), which every proc-macro pulls in. It is a permissive data licence.
  - `CC0-1.0`: the licence of `notify` 8.2.0 (checked in its crate manifest), which spec 003's watcher needs. It is a public-domain dedication.
  - `MPL-2.0` is allowed as a dependency licence because its copyleft is per file: using an unmodified MPL crate imposes nothing on wolluf's own files. Modifying MPL files would require publishing those files, so vendored MPL code is not allowed without a new ADR.
- `[licenses.private] ignore = true` while `publish = false`, so the workspace's own crates are not checked against the allowlist.
- **F4 audio:** the stack stays permissive. LGPL is only needed for mp3 encoding (LAME, shine) or SoundTouch (research 03 l.188), so those are excluded in-process.
- The third-party licence bundle for installers belongs to the release spec (F4).

## Alternatives considered
- **A copyleft project licence (GPL-3.0).** It would allow porting from `interlude/` and linking tosu. Not chosen: the user picked MIT on 2026-09-28, which keeps reuse and later distribution simple. The GPL sources stay reference-only.
- **Apache-2.0 or dual MIT/Apache-2.0.** Reasonable, but MIT was the user's explicit choice. The allowlist accepts Apache-2.0 dependencies either way.
- **Porting Sunny from the Python repository and asking for a licence later.** Rejected: the code is all rights reserved today, and a clean-room implementation from the paper removes the dependency on the author's answer.
- **Linking a memory-reader library in-process.** Rejected: tosu and gosumemory are LGPL/GPL, and live reading is optional anyway, since failed plays are saved to scores.db on client 20260924 (research 03 l.115).
- **The architecture's allowlist without `Unicode-3.0` and `CC0-1.0`.** Rejected: nothing with a proc-macro would build, and 003's watcher would fail the gate.

## Consequences
- `cargo deny check` fails CI the moment a disallowed licence enters the tree, and adding a licence to the allowlist needs an ADR.
- Every port PR also touches `NOTICE`, and reviewers reject a port without it.
- Sunny work in F1 costs more (reading the paper, validating against oracles) than porting would.
- Live data (tosu) is limited to what its websocket exposes, and it stays optional.
