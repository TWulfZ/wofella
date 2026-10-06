import { useTranslation } from "react-i18next";
import type { ProfileEntryDto } from "@/ipc/bindings";
import { entryScopeParam, useActiveEntry } from "../activeEntry";

function entryLabel(entry: ProfileEntryDto, t: (key: string) => string): string {
  return entry.profileKind === "all_players" ? t("players.scope.allPlayers") : entry.label;
}

export function ScopePicker() {
  const { t } = useTranslation();
  const { entries, active, setSearch } = useActiveEntry();
  if (entries.length === 0) {
    return null;
  }
  return (
    <div className="flex items-center gap-2 text-sm">
      <select
        aria-label={t("players.scope.label")}
        className="border-input bg-surface-raised text-foreground hover:border-primary/60 focus-visible:ring-ring h-9 cursor-pointer rounded-md border px-3 text-sm transition-colors focus-visible:ring-2 focus-visible:outline-none"
        value={active === undefined ? "" : entryScopeParam(active)}
        onChange={(e) => {
          const next = entries.find((entry) => entryScopeParam(entry) === e.target.value);
          if (next !== undefined) {
            setSearch({ scope: entryScopeParam(next) });
          }
        }}
      >
        {entries.map((entry) => (
          <option key={entryScopeParam(entry)} value={entryScopeParam(entry)}>
            {entryLabel(entry, t)}
          </option>
        ))}
      </select>
      {active?.profileKind === "self" && active.aliasIds.length === 0 && (
        <span className="text-muted-foreground">{t("players.scope.noNames")}</span>
      )}
    </div>
  );
}
