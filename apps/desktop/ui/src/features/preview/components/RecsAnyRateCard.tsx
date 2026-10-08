import { useQuery } from "@tanstack/react-query";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { useErrorText } from "@/ipc/errorText";
import { Card, CardContent } from "@/shared/ui/card";
import { Switch } from "@/shared/ui/switch";
import { recsAnyRateQuery, useSetRecsAnyRate } from "../queries";

/** `preview.recs.any_rate` (ADR 0024 Recommendations): widens picks from stable's mod rates to the whole grid. */
export function RecsAnyRateCard() {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const titleId = useId();
  const switchId = useId();
  const helpId = useId();
  const stored = useQuery(recsAnyRateQuery());
  const save = useSetRecsAnyRate();
  const error = stored.error ?? save.error;
  return (
    <Card role="region" aria-labelledby={titleId}>
      <CardContent className="flex flex-col gap-3">
        <span id={titleId} className="font-display text-base leading-snug font-semibold">
          {t("settings.recs.title")}
        </span>
        <div className="flex items-start justify-between gap-6">
          <div className="flex min-w-0 flex-col gap-1">
            <label htmlFor={switchId} className="text-sm font-medium">
              {t("settings.recs.anyRate")}
            </label>
            <p id={helpId} className="text-muted-foreground text-sm">
              {t("settings.recs.anyRateHelp")}
            </p>
            {error !== null && (
              <p role="alert" className="text-destructive text-sm">
                {errorText(error)}
              </p>
            )}
          </div>
          <Switch
            id={switchId}
            aria-describedby={helpId}
            checked={stored.data ?? false}
            disabled={stored.data === undefined || save.isPending}
            onCheckedChange={(on) => {
              save.mutate(on);
            }}
            className="mt-1"
          />
        </div>
      </CardContent>
    </Card>
  );
}
