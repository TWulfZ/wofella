# <Feature title>

Branch `<type>/<feature-name>` from `main` @ <short sha> · opened <YYYY-MM-DD>

## Objective
One sentence: what exists when this is done, seen from the user or caller.

## Problem and why
The need, with evidence (architecture §x, research file and line, a measurement). Name the rejected alternative only when the choice was real.

## Scope
- Authorized: <paths, crates, behaviours>
- Out of scope: <what a reader might expect here that this work does not do>

## Constraints
- <rule this work must respect> (<D-rule | ADR NNNN | research file l.N>)
- Depends on: <feature or F0 item, merged | in flight>
- TDD: strict (docs/conventions.md). Runner: `<exact command>`
- Delivery: ~<N> authored lines forecast

## Acceptance criteria
- <observable outcome> → `<test name or command that proves it>`

## Tasks
- [ ] T1 — <behaviour>. Acceptance: <observable> (`<proof>`). Route: <inline | delegated: trigger>. Tier: <passive | medium | high>. Commit: —

## Progress
- <YYYY-MM-DD> T1: RED `<observed failure>` → GREEN. `<command>`: <result>. Spot check: `<command>` ok. Pre-existing failures: <none | named, with evidence>.
- <YYYY-MM-DD> Accepted change: <what> — <why>.

## Next step
The one thing to do next, precise enough to resume cold.
