import { CircleCheck } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { type ChartWindow, Playfield, useLoadedSkin } from "@/features/playfield";
import type { SkinDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/shared/ui/card";
import type { SkinOption } from "./skinChoice";

// Folder names are directory names, so they are never empty.
const NONE_VALUE = "";

export interface DefaultSkinCardProps {
  options: readonly SkinOption[];
  /** Null is the procedural look. */
  folder: string | null;
  /** False until the skin list answers, so a stored choice is never shown against a list that cannot hold it. */
  ready: boolean;
  /** The selected folder's skin; null for None or while it loads. */
  skin: SkinDto | null;
  /** Undefined while the sample loads; null when there is none to draw. */
  example: ChartWindow | null | undefined;
  error: unknown;
  onChange: (folder: string | null) => void;
}

/**
 * Data comes in through props because the skin and example queries belong to the label slice, which already imports
 * this one; the route wires them so the two slices do not import each other.
 */
export function DefaultSkinCard({ options, folder, ready, skin, example, error, onChange }: DefaultSkinCardProps) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const titleId = useId();
  const selectId = useId();
  const [saved, setSaved] = useState(false);
  const loaded = useLoadedSkin(skin);
  const drawn = loaded.skin;
  const skinProps = drawn === null ? {} : { skin: drawn, hitPosition: drawn.hitPosition, columnWidths: drawn.columnWidth };
  const skinName = folder ?? t("settings.defaultSkin.none");

  return (
    <Card role="region" aria-labelledby={titleId}>
      <CardHeader>
        <CardTitle id={titleId}>{t("settings.defaultSkin.title")}</CardTitle>
        <CardDescription>{t("settings.defaultSkin.description")}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-6 sm:grid-cols-[minmax(0,1fr)_minmax(0,16rem)]">
        <div className="flex flex-col gap-3">
          <label htmlFor={selectId} className="sr-only">
            {t("settings.defaultSkin.title")}
          </label>
          <select
            id={selectId}
            value={folder ?? NONE_VALUE}
            disabled={!ready}
            onChange={(e) => {
              onChange(e.target.value === NONE_VALUE ? null : e.target.value);
              setSaved(true);
            }}
            className="border-input bg-background focus-visible:ring-ring focus-visible:ring-offset-card h-9 w-full max-w-sm rounded-md border px-2 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
          >
            <option value={NONE_VALUE}>{t("settings.defaultSkin.none")}</option>
            {options.map(({ entry, hasKeymode }) => (
              <option key={entry.folder} value={entry.folder}>
                {hasKeymode ? entry.folder : t("settings.defaultSkin.defaults", { folder: entry.folder })}
              </option>
            ))}
          </select>
          {error !== null && error !== undefined && (
            <p role="alert" className="text-destructive text-sm">
              {errorText(error)}
            </p>
          )}
          {loaded.failedSlots.length > 0 && (
            <p className="text-muted-foreground text-sm">
              {t("settings.defaultSkin.decodeFailed", { slots: loaded.failedSlots.join(", ") })}
            </p>
          )}
          {saved && (
            <p role="status" className="text-success flex items-center gap-1.5 text-sm">
              <CircleCheck aria-hidden="true" className="size-4" />
              {t("settings.defaultSkin.saved")}
            </p>
          )}
        </div>
        {example === undefined ? (
          <div aria-busy="true" className="bg-surface-raised h-64 rounded-lg motion-safe:animate-pulse" />
        ) : example === null ? (
          <div className="bg-surface-raised text-muted-foreground grid h-64 place-items-center rounded-lg px-4 text-center text-sm">
            {t("settings.defaultSkin.previewUnavailable")}
          </div>
        ) : (
          <div
            role="img"
            aria-label={t("settings.defaultSkin.preview", { skin: skinName })}
            className="bg-surface-raised ring-border h-64 overflow-hidden rounded-lg ring-1"
          >
            <Playfield window={example} clock={null} scroll="fit" {...skinProps} className="h-full w-full" />
          </div>
        )}
      </CardContent>
    </Card>
  );
}
