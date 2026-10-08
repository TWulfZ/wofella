import { useQuery } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Button } from "@/shared/ui/button";
import { keymodesQuery } from "../queries";
import { DEFAULT_KEYMODE, validateGlobalSearch } from "../search";

// Lists only what meta_keymodes returns, so a keymode the engine registry enables shows up without a UI change (D5).
export function KeymodeSwitcher() {
  const { t } = useTranslation();
  const keymodes = useQuery(keymodesQuery());
  const active = validateGlobalSearch(useSearch({ strict: false })).keymode ?? DEFAULT_KEYMODE;
  const navigate = useNavigate();
  const listed = keymodes.data ?? [];
  if (listed.length < 2) {
    return null;
  }
  return (
    <div
      role="group"
      aria-label={t("players.keymodeSwitcher.label")}
      className="border-input bg-surface-raised flex h-9 shrink-0 items-center gap-1 rounded-md border p-1"
    >
      {listed.map(({ keymode }) => (
        <Button
          key={keymode}
          size="sm"
          variant="ghost"
          className="text-muted-foreground hover:text-foreground focus-visible:ring-ring aria-pressed:bg-primary aria-pressed:text-primary-foreground aria-pressed:hover:bg-primary/90 tabular cursor-pointer rounded-sm transition-colors focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-surface-raised focus-visible:outline-none"
          aria-pressed={active === keymode}
          onClick={() => {
            void navigate({ to: ".", search: (prev: Record<string, unknown>) => ({ ...prev, keymode }) });
          }}
        >
          {t("players.keymodeSwitcher.option", { keymode })}
        </Button>
      ))}
    </div>
  );
}
