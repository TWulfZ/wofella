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

/** A stored choice wins; one naming a skin that is gone falls back to the cfg's active skin. */
export function selectedSkin(list: SkinListDto | undefined, choice: SkinChoice | undefined): SkinEntryDto | null {
  if (list === undefined) {
    return null;
  }
  const find = (folder: string): SkinEntryDto | null => list.skins.find((s) => s.folder === folder) ?? null;
  if (choice !== undefined) {
    if (choice.folder === null) {
      return null;
    }
    const chosen = find(choice.folder);
    if (chosen !== null) {
      return chosen;
    }
  }
  return list.current === null ? null : find(list.current);
}
