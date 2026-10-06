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
    <div
      role="group"
      aria-label={t("players.merge.label")}
      className="border-input bg-surface-raised flex h-9 items-center gap-1 rounded-md border p-1"
    >
      {MODES.map((mode) => (
        <Button
          key={mode}
          size="sm"
          variant="ghost"
          className="text-muted-foreground hover:text-foreground focus-visible:ring-ring aria-pressed:bg-primary aria-pressed:text-primary-foreground aria-pressed:hover:bg-primary/90 cursor-pointer rounded-sm transition-colors focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-surface-raised focus-visible:outline-none"
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
