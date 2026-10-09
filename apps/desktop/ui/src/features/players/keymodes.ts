import type { KeymodeDto } from "@/ipc/bindings";

/**
 * An unknown list (still loading or failed) counts as capable, so a slow or broken meta_keymodes never locks 7K
 * labelling; a known list without the keymode means the engine registry does not enable it.
 */
export function keymodeHasPatterns(keymodes: readonly KeymodeDto[] | undefined, keymode: number): boolean {
  if (keymodes === undefined) {
    return true;
  }
  return keymodes.find((k) => k.keymode === keymode)?.hasPatterns ?? false;
}

export interface PatternAvailability {
  patterns: boolean;
  /** Keymodes that can be labelled instead; empty when `patterns` holds. */
  alternatives: number[];
}

export function patternAvailability(keymodes: readonly KeymodeDto[] | undefined, keymode: number): PatternAvailability {
  if (keymodeHasPatterns(keymodes, keymode)) {
    return { patterns: true, alternatives: [] };
  }
  return { patterns: false, alternatives: (keymodes ?? []).filter((k) => k.hasPatterns).map((k) => k.keymode) };
}
