# 0001 Record architecture decisions

- Status: Accepted
- Date: 2026-09-28

## Context
wolluf is built by one developer, with agents doing much of the implementation. `docs/architecture.md` is binding, but it states the current design, not why the design beat the alternatives. When the reasons are missing, a later change either reopens a settled question or quietly breaks an invariant nobody wrote down. Several F0 decisions pick between real options: the storage split, the crate DAG, the identity rule, the IPC generator and the licence policy. Some of them also depart from the baseline architecture (in-memory snapshots, opener instead of shell). Each of those needs a durable record that code review can check against.

## Decision
- Decisions are recorded as ADRs in `docs/adr/NNNN-kebab-title.md` using the MADR-style template in `.claude/skills/wolluf-sdd/templates/adr.md`. Every ADR has a `Status`, a `Date` and the headings Context, Decision, Alternatives considered and Consequences.
- Numbers are four digits, assigned in order and never reused. A number reserved in architecture §11 (0010 canonical accuracy curve, 0011 LN judging confidence, 0012 `.osg` handling, 0013 param packs, signing and eval gate) stays reserved for that topic.
- The statuses are `Proposed`, `Accepted` and `Superseded by NNNN`. An accepted ADR is not rewritten to say something different. A change of mind is a new ADR that supersedes it. The one exception is an **amendment**: an appended, dated section that narrows or replaces one rule and keeps the rest of the record. It is used only when the original ADR's context still holds, for example the 0005 amendment by spec 004.
- An ADR is **required** for:
  - a new crate dependency edge, or a change to `xtask/layers.toml` (D1);
  - a storage change: user.db schema semantics, cache.db policy, the vault, or any persisted encoding (ids, version keys);
  - any change to the meaning or scale of θ (§9.4 case B);
  - a cross-cutting rule that more than one crate must obey (errors, determinism, lint policy, privacy, licensing);
  - a choice between real alternatives where the losing option is plausible enough that someone will propose it again;
  - any deviation from `docs/architecture.md`. The ADR and the architecture edit land in the same PR.
- An ADR is not needed for a choice local to one module that the code and its spec already explain.
- Evidence is cited, never recalled: research files with line numbers, measurements with their date, and upstream sources with the date they were fetched.
- A spec links the ADRs it relies on. An ADR links the spec that produced it.

## Alternatives considered
- **Architecture document only.** It would mix the current design with its history and grow without bound. It also makes supersession invisible, so nobody can tell whether a sentence is still binding.
- **Decisions in PR descriptions or commit bodies.** They are hard to find, not versioned alongside the code they constrain, and the commit policy keeps bodies rare on purpose.
- **Heavier formats (full MADR with pros/cons matrices, RFC process).** Too much overhead for a solo project. The four headings capture what a reviewer needs.

## Consequences
- `cargo xtask check-layers` failures caused by a `layers.toml` change point to an ADR, so a reviewer can check an edge against its rationale.
- ADRs 0001–0009 are written in F0 from the baseline architecture. Later ADRs are written when their decision is made, not in advance.
- The architecture document can stay a readable statement of "what is", because the "why" and the "why not" live here.
- Writing an ADR costs a little time on every cross-cutting change, and that cost is intended.

## Amendment 2026-09-29: ODD replaces per-feature specs
The context still holds; only the coupling to specs changes. From F1 on, work follows the `wolluf-odd` skill: small work leaves no document, and a substantial feature keeps one transient `odd/tasks/<feature-name>.md` that is removed at close. The permanent spec + tasks pair was dropped because it restated ADRs and code. Keeping finished feature documents, as upstream Gentle-AI ODD does, was rejected for the same reason.
- The template is `.claude/skills/wolluf-odd/templates/adr.md`.
- "An ADR is not needed for a choice local to one module that the code and its spec already explain" now reads "…that the code and its tests already explain".
- "A spec links the ADRs it relies on. An ADR links the spec that produced it." is replaced by: an ADR cites its evidence directly (research lines, measurements, commits) and never links an `odd/tasks/` document. ADRs written in F0 keep their spec links; `docs/specs/` stays as the F0 record.
