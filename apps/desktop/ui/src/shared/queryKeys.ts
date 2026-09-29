// Query-key roots match the domains carried by DataChanged, so one invalidation hits exactly one slice (§8).
export type QueryDomain = "setup" | "jobs" | "players" | "plays";

// §8 appends scopeHash and manifestHash; F0 has no engine manifest yet, so F1 adds it here in one place.
export function qk<const A extends readonly unknown[]>(domain: QueryDomain, ...args: A): readonly [QueryDomain, ...A] {
  return [domain, ...args];
}
