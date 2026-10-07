import { RotateCw } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { SkinOption } from "@/features/preferences";
import { Button } from "@/shared/ui/button";

// Folder names are directory names, so they are never empty.
const NONE_VALUE = "";

export interface SkinPickerProps {
  options: readonly SkinOption[];
  /** Null is the procedural look. */
  folder: string | null;
  ready: boolean;
  reloading: boolean;
  onChange: (folder: string | null) => void;
  onReload: () => void;
}

export function SkinPicker({ options, folder, ready, reloading, onChange, onReload }: SkinPickerProps) {
  const { t } = useTranslation();
  const id = useId();
  return (
    <div className="flex flex-col gap-1.5 text-sm">
      <label htmlFor={id} className="text-foreground/90">
        {t("label.skin.label")}
      </label>
      <div className="flex items-center gap-1.5">
        <select
          id={id}
          value={folder ?? NONE_VALUE}
          disabled={!ready}
          onChange={(e) => {
            onChange(e.target.value === NONE_VALUE ? null : e.target.value);
          }}
          className="border-input bg-background/80 h-8 min-w-0 flex-1 rounded-md border px-2 text-sm"
        >
          <option value={NONE_VALUE}>{t("label.skin.none")}</option>
          {options.map(({ entry, hasKeymode }) => (
            <option key={entry.folder} value={entry.folder}>
              {hasKeymode ? entry.folder : t("label.skin.defaults", { folder: entry.folder })}
            </option>
          ))}
        </select>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={t("label.skin.reload")}
          title={t("label.skin.reload")}
          disabled={reloading}
          onClick={onReload}
        >
          <RotateCw aria-hidden />
        </Button>
      </div>
    </div>
  );
}
