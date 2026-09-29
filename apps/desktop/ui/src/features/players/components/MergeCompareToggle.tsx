import { useTranslation } from "react-i18next";
import type { MergeModeDto } from "@/ipc/bindings";
import { Button } from "@/shared/ui/button";
import { useActiveEntry } from "../activeEntry";

const MODES: readonly MergeModeDto[] = ["merged", "separate"];

export function MergeCompareToggle() {
  const { t } = useTranslation();
  const { active, mergeMode, setSearch } = useActiveEntry();
  // Comparing needs at least two names (spec 004 Behaviour 8).
  if (active === undefined || active.aliasIds.length <= 1) {
    return null;
  }
  return (
    <div role="group" aria-label={t("players.merge.label")} className="flex gap-1">
      {MODES.map((mode) => (
        <Button
          key={mode}
          size="sm"
          variant={mergeMode === mode ? "secondary" : "ghost"}
          aria-pressed={mergeMode === mode}
          onClick={() => {
            setSearch({ merge: mode });
          }}
        >
          {t(`players.merge.${mode}`)}
        </Button>
      ))}
    </div>
  );
}
