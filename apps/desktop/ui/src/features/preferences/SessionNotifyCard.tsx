import { useQuery } from "@tanstack/react-query";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { useErrorText } from "@/ipc/errorText";
import { Card, CardContent } from "@/shared/ui/card";
import { Switch } from "@/shared/ui/switch";
import { sessionNotifyQuery, useSetSessionNotify } from "./queries";

/** Opt-in (ADR 0020): a finished map flashes the taskbar entry; there is no OS notification to configure. */
export function SessionNotifyCard() {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const titleId = useId();
  const descriptionId = useId();
  const stored = useQuery(sessionNotifyQuery());
  const save = useSetSessionNotify();
  const error = stored.error ?? save.error;
  return (
    <Card role="region" aria-labelledby={titleId}>
      <CardContent className="flex items-start justify-between gap-6">
        <div className="flex min-w-0 flex-col gap-1">
          <span id={titleId} className="font-display text-base leading-snug font-semibold">
            {t("settings.sessionNotify.title")}
          </span>
          <p id={descriptionId} className="text-muted-foreground text-sm">
            {t("settings.sessionNotify.description")}
          </p>
          {error !== null && (
            <p role="alert" className="text-destructive text-sm">
              {errorText(error)}
            </p>
          )}
        </div>
        <Switch
          aria-labelledby={titleId}
          aria-describedby={descriptionId}
          checked={stored.data ?? false}
          disabled={stored.data === undefined || save.isPending}
          onCheckedChange={(on) => {
            save.mutate(on);
          }}
          className="mt-1"
        />
      </CardContent>
    </Card>
  );
}
