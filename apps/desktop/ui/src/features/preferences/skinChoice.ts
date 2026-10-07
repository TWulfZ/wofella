// The default skin is a per-viewer convenience: a blocked or cleared storage must leave every screen on the cfg skin.
// Preferences owns it like the hand layout; the Label screen reads the same key (features/label/prefs.ts).

import type { ChartWindow } from "@/features/playfield";
import type { SkinEntryDto, SkinListDto } from "@/ipc/bindings";

/** The Label screen's key since before Settings had this card; renaming it would forget every stored choice. */
export const SKIN_CHOICE_KEY = "wolluf.label.skin";

/** `folder: null` is the procedural look; no stored choice follows the cfg's active skin. */
export interface SkinChoice {
  folder: string | null;
}

export function readSkinChoice(): SkinChoice | undefined {
  let raw: string | null;
  try {
    raw = globalThis.localStorage.getItem(SKIN_CHOICE_KEY);
  } catch {
    return undefined;
  }
  if (raw === null) {
    return undefined;
  }
  try {
    // JSON so a skin folder literally named like a sentinel cannot be mistaken for "None".
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed === "object" && parsed !== null && "folder" in parsed) {
      const { folder } = parsed;
      if (folder === null || typeof folder === "string") {
        return { folder };
      }
    }
  } catch {
    // Unreadable: treated as never chosen.
  }
  return undefined;
}

export function writeSkinChoice(choice: SkinChoice): void {
  try {
    globalThis.localStorage.setItem(SKIN_CHOICE_KEY, JSON.stringify({ folder: choice.folder }));
  } catch {
    // Not persisted; the in-memory choice still applies for this session.
  }
}

/** Skins without a block for the keymode still load, with skin.ini's defaults for every column (research 06). */
export interface SkinOption {
  entry: SkinEntryDto;
  hasKeymode: boolean;
}

export function skinOptions(list: SkinListDto | undefined, keymode: number): SkinOption[] {
  const options = (list?.skins ?? []).map((entry) => ({ entry, hasKeymode: entry.keymodes.includes(keymode) }));
  return [...options.filter((o) => o.hasKeymode), ...options.filter((o) => !o.hasKeymode)];
}

/**
 * The folder to fetch: a stored choice wins and needs no list, so its skin loads while the slow listing runs; one
 * naming a skin that is gone falls back to the cfg's active skin once the list arrives.
 */
export function selectedSkinFolder(list: SkinListDto | undefined, choice: SkinChoice | undefined): string | null {
  if (list === undefined) {
    return choice?.folder ?? null;
  }
  const listed = (folder: string): boolean => list.skins.some((s) => s.folder === folder);
  if (choice !== undefined) {
    if (choice.folder === null || listed(choice.folder)) {
      return choice.folder;
    }
  }
  return list.current !== null && listed(list.current) ? list.current : null;
}

export const SKIN_PREVIEW_PARAMS = {
  /** Chords over every column show the skin's note art per column in one still frame; any example stands in if absent. */
  patternId: "regular.stream.jumpstream",
} as const;

export function previewExample(examples: ReadonlyMap<string, ChartWindow> | undefined): ChartWindow | undefined {
  if (examples === undefined) {
    return undefined;
  }
  return examples.get(SKIN_PREVIEW_PARAMS.patternId) ?? examples.values().next().value;
}
