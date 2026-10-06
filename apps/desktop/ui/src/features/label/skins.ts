import type { SkinEntryDto, SkinListDto } from "@/ipc/bindings";
import type { SkinChoice } from "./prefs";

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
