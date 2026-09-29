import type { MergeModeDto } from "@/ipc/bindings";

/** `self` | `p:<profileId>` | `all` (spec 004 Routes). */
export type ScopeParam = "self" | "all" | `p:${number}`;

// `| undefined` is deliberate: the router merges validated params over the raw ones, so an invalid value is only
// dropped when the validator overwrites its key with undefined.
export interface GlobalSearch {
  scope?: ScopeParam | undefined;
  keymode?: number | undefined;
  merge?: MergeModeDto | undefined;
}

// The MVP keymode (CLAUDE.md): views without ?keymode= show 7K.
export const DEFAULT_KEYMODE = 7;
// Keymode buckets run k1..k16 (spec 004 KeymodeBucket).
const MIN_KEYMODE = 1;
const MAX_KEYMODE = 16;

const PROFILE_SCOPE = /^p:([1-9]\d*)$/;
const MERGE_MODES: readonly MergeModeDto[] = ["merged", "separate"];

function parseScope(value: unknown): ScopeParam | undefined {
  if (value === "self" || value === "all") {
    return value;
  }
  if (typeof value === "string" && PROFILE_SCOPE.test(value)) {
    return value as ScopeParam;
  }
  return undefined;
}

function parseKeymode(value: unknown): number | undefined {
  const n = typeof value === "string" && value.trim() !== "" ? Number(value) : value;
  return typeof n === "number" && Number.isInteger(n) && n >= MIN_KEYMODE && n <= MAX_KEYMODE ? n : undefined;
}

function parseMerge(value: unknown): MergeModeDto | undefined {
  return MERGE_MODES.find((m) => m === value);
}

/**
 * The root route's validateSearch delegates here (spec 005 Router). Invalid values are dropped rather than rejected,
 * so a stale or hand-edited URL still opens the view with its defaults.
 */
export function validateGlobalSearch(search: Record<string, unknown>): GlobalSearch {
  return {
    scope: parseScope(search["scope"]),
    keymode: parseKeymode(search["keymode"]),
    merge: parseMerge(search["merge"]),
  };
}

export function profileScopeParam(id: number): ScopeParam {
  return `p:${id}`;
}
