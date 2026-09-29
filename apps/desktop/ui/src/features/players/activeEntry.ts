import { useQuery } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
import type { MergeModeDto, ProfileEntryDto } from "@/ipc/bindings";
import { profilesQuery } from "./queries";
import { DEFAULT_KEYMODE, profileScopeParam, validateGlobalSearch, type GlobalSearch, type ScopeParam } from "./search";

export function entryScopeParam(entry: ProfileEntryDto): ScopeParam {
  if (entry.ref.kind === "all_players") {
    return "all";
  }
  return entry.profileKind === "self" ? "self" : profileScopeParam(entry.ref.id);
}

/** Which listed entry the URL names; a missing or unknown scope falls back to the default profile (spec 004 B9). */
export function resolveActiveEntry(entries: readonly ProfileEntryDto[], scope: ScopeParam | undefined) {
  const named = scope === undefined ? undefined : entries.find((e) => entryScopeParam(e) === scope);
  return named ?? entries.find((e) => e.isDefault) ?? entries.find((e) => e.profileKind === "self");
}

export interface ActiveEntry {
  search: GlobalSearch;
  keymode: number;
  entries: readonly ProfileEntryDto[];
  active: ProfileEntryDto | undefined;
  mergeMode: MergeModeDto | undefined;
  setSearch: (patch: Partial<GlobalSearch>) => void;
}

export function useActiveEntry(): ActiveEntry {
  // Validated again here so the controls work under any parent route, not only below the root validateSearch.
  const search = validateGlobalSearch(useSearch({ strict: false }));
  const navigate = useNavigate();
  const keymode = search.keymode ?? DEFAULT_KEYMODE;
  const profiles = useQuery(profilesQuery(keymode));
  const entries = profiles.data ?? [];
  const active = resolveActiveEntry(entries, search.scope);
  return {
    search,
    keymode,
    entries,
    active,
    // The URL override wins; for All players it is the only mode source (spec 004 Behaviour 8).
    mergeMode: search.merge ?? active?.mergeMode,
    setSearch: (patch) => {
      void navigate({ to: ".", search: (prev: Record<string, unknown>) => ({ ...prev, ...patch }) });
    },
  };
}
