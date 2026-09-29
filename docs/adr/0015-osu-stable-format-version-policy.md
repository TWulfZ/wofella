# 0015 osu! stable format version policy

- Status: Accepted
- Date: 2026-09-28

## Context
osu!.db, scores.db, collection.db and `.osr` all start with an `Int` version. Architecture §7 says "unknown DB format versions are a hard, explicit error; the parser never guesses". Taken literally, "unknown" would mean "any version wolluf has not seen", and that breaks in practice:
- **The header is the client build that last wrote the file, not a format revision.** It changes on every osu! update, even when the layout does not. On the pilot, osu!.db and scores.db are at 20260924 (the client build), while collection.db is at 20260624 because no collection has changed since then. scores.db per-score versions span 20220424 … 20260924, five builds in six months (spec 002, measured 2026-09-28).
- **Real layout changes are rare and documented.** The osu!.db entry-size int disappears from 20191106, star-rating pairs change from Int-Double to Int-Float at 20250107, and byte-sized AR/CS/HP/OD exist below 20140609 (`research/scripts/rejudge/legacy_db.md`; research `01-landscape-verified.txt` l.83). The score online id is absent below 20121008, an `i32` from 20121008 and an `i64` from 20140721 (lazer `LegacyScoreDecoder`, fetched 2026-09-28).
- **The formats are self-checking enough to detect a layout change.** Tagged strings (`0x00`/`0x0b`), star-rating pair tags (`0x08` followed by `0x0d` or `0x0c`), the scores.db −1 marker after the timestamp, counts bounded by the remaining bytes, and exact EOF all fail loudly on a misaligned read (spec 002, Behaviour).
- A silently mis-parsed file would corrupt the irreplaceable ledger (ADR 0003). Refusing to sync after every osu! update would make wolluf unusable until a new wolluf release.

## Decision
Each format kind (`osu_db`, `scores_db`, `collection_db`, `osr`; 006 appends `osg`) has a `VersionPolicy { min, newest_verified }`, and every header version is classified into one of four classes:

| Class | Condition | Behaviour |
|---|---|---|
| `TooOld` | version < `min` | `UnsupportedFormat { kind, version }` at once |
| `Verified` | `min` ≤ version ≤ `newest_verified` | Decoded with the layout the thresholds select |
| `Unverified` | version > `newest_verified` (and below the lazer range) | Decoded with the newest layout **only if every structural invariant holds**, including exact EOF (for `.osr`: exactly the expected trailing bytes after the payload). The result carries the diagnostic `format.unverified_version`. **Any** invariant failure on such a file is reported as `UnsupportedFormat`, not `PARSE_FAILED`, because the likeliest cause is a layout change |
| `Lazer` | score/replay version ≥ 30000000 | `UnsupportedFormat` (lazer exports are out of scope) |

F0 constants (spec 002, `codec::version`):
- osu!.db: min 20140609, entry size removed at 20191106, Int-Float pairs from 20250107, newest verified 20260924;
- scores.db: min 20140609, newest verified 20260924;
- collection.db: min 20140609, newest verified 20260624;
- score and replay online id thresholds: 20121008 (`i32`) and 20140721 (`i64`);
- lazer range: from 30000000.

**Error mapping.** `UnsupportedFormat` → `ErrorCode::UNSUPPORTED_FORMAT`. All other codec errors (truncation, bad tags, unexpected values, count or EOF violations on a `Verified` file) → `PARSE_FAILED`. Decoders are all-or-nothing per file: there is no partial result.

**Definition of "unknown" for §7.** A version is unknown when it is `TooOld`, `Lazer`, or `Unverified` and structurally invalid. The §7 sentence gets this definition in 002's close-out architecture edit (002 T17).

**Raising `newest_verified`.** A newer build is promoted to verified when the corpus harness (spec 002 AC14: oracle parity and byte-identical round trips) passes on a real file of that version. A new layout threshold is added only with evidence (a documented format change or a failing corpus file), never by guessing.

## Alternatives considered
- **Strict reading of §7: reject every version above the newest verified one.** Rejected by the user (2026-09-28). It would break sync on every osu! update, roughly monthly on the pilot, until a wolluf release. The user would learn to ignore the error, which defeats its purpose.
- **Lenient decoding of any version with no structural gate.** Rejected: a real layout change would be mis-parsed silently into the ledger. The structural invariants plus the `UnsupportedFormat` escalation are what make leniency safe.
- **Reporting structural failures on unverified files as `PARSE_FAILED`.** Rejected: the UI would suggest a corrupt file when the true cause is most likely a new format. `UNSUPPORTED_FORMAT` tells the user to update wolluf.
- **Detecting the layout by trial (try the old layout, then the new one).** Rejected: two layouts could both parse by accident, and "the parser never guesses" (§7).

## Consequences
- Sync keeps working across osu! updates that do not change a layout, with a visible warning until the build is verified.
- A real layout change fails loudly as `UNSUPPORTED_FORMAT` before anything reaches the ledger.
- The newest-verified constants need upkeep, driven by the corpus harness. Forgetting to raise them costs only a warning.
- The `TooOld` and pre-20191106 branches have no real corpus source. They are covered by the `test-support` encoders (spec 002). The pre-20140721 online-id width is untested on real data (the pilot's oldest record is 20220424), but the −1 marker and exact-EOF checks make a wrong width fail loudly.
