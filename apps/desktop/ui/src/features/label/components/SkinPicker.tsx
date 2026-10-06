import { RotateCw } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/shared/ui/button";
import type { SkinOption } from "../skins";

// Folder names are directory names, so they are never empty.
const NONE_VALUE = "";

interface SkinPickerProps {
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
    <div className="flex items-center gap-2 text-sm">
      <label htmlFor={id} className="text-muted-foreground">
        {t("label.skin.label")}
      </label>
      <select
        id={id}
        value={folder ?? NONE_VALUE}
        disabled={!ready}
        onChange={(e) => {
          onChange(e.target.value === NONE_VALUE ? null : e.target.value);
        }}
        className="border-input bg-background h-7 max-w-56 rounded-md border px-1.5 text-sm"
      >
        <option value={NONE_VALUE}>{t("label.skin.none")}</option>
        {options.map(({ entry, hasKeymode }) => (
          <option key={entry.folder} value={entry.folder}>
            {hasKeymode ? entry.folder : t("label.skin.defaults", { folder: entry.folder })}
          </option>
        ))}
      </select>
      <Button variant="ghost" size="sm" disabled={reloading} onClick={onReload}>
        <RotateCw aria-hidden="true" />
        {t("label.skin.reload")}
      </Button>
    </div>
  );
}
