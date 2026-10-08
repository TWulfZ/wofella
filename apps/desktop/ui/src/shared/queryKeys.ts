// Query-key roots match the domains carried by DataChanged, so one invalidation hits exactly one slice (§8).
export type QueryDomain = "setup" | "jobs" | "players" | "plays" | "labels" | "library" | "skins" | "meta";

// §8 appends scopeHash and manifestHash; F0 has no engine manifest yet, so F1 adds it here in one place.
export function qk<const A extends readonly unknown[]>(domain: QueryDomain, ...args: A): readonly [QueryDomain, ...A] {
  return [domain, ...args];
}

// Owned by the label progress slice, but the session-play-added bridge in ipc/ invalidates them and may not import a
// feature (D14), so the roots live here.
export const SESSION_PLAYS_ROOT = qk("plays", "session");
export const LABEL_PROGRESS_ROOT = qk("labels", "progress");
