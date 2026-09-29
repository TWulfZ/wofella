# <NNN> <Title>

Status: Draft | In progress | Done
Phase: F<n> · Owner: <name> · Date: <YYYY-MM-DD>
Links: architecture §<x>, ADR <nnnn>, research <file>

## Problem
Why this is needed. One paragraph, user-visible where possible.

## Scope
- In:
- Out (non-goals):

## Behaviour
What the user or caller observes. Include edge cases.

## Domain rules
Facts the code must respect, each with a citation (research file or ADR).

## Design
- Crates and modules touched. Only allowed dependency edges (architecture §4).
- New types and traits, in prose.
- Versioned stages affected (`VERSION` bumps) and param-pack sections.

## Data
- user.db migration: yes/no (tables).
- cache.db change: yes/no (bump `CACHE_SCHEMA_VERSION`).
- Vault or blob changes.

## IPC / UI
Commands, DTOs, events, routes, or "none".

## Acceptance criteria
- [ ] AC1: observable, testable statement → test or command that proves it.

## Risks / open questions

## Deviations (filled at close)
