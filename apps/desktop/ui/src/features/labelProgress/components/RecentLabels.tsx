import { useId } from "react";
import { useTranslation } from "react-i18next";
import { patternName } from "@/features/label";
import type { RecentLabelDto } from "@/ipc/bindings";
import { formatDateTime, formatRelative } from "@/shared/format";
import { Badge } from "@/shared/ui/badge";

interface RecentLabelsProps {
  recent: readonly RecentLabelDto[] | undefined;
  nowMs: number;
  unavailable: boolean;
}

export function RecentLabels({ recent, nowMs, unavailable }: RecentLabelsProps) {
  const { t, i18n } = useTranslation();
  const headingId = useId();
  return (
    <section aria-labelledby={headingId} className="bg-card ring-border flex flex-col gap-3 rounded-xl p-5 shadow-lg shadow-black/20 ring-1">
      <h2 id={headingId} className="font-display text-base font-semibold">
        {t("labelProgress.recent.title")}
      </h2>
      {recent === undefined ? (
        <p className="text-muted-foreground text-sm">{t(unavailable ? "labelProgress.unavailable" : "common.loading")}</p>
      ) : recent.length === 0 ? (
        <p className="text-muted-foreground text-sm">{t("labelProgress.recent.empty")}</p>
      ) : (
        <ol className="flex flex-col divide-y">
          {recent.map((label) => (
            <li key={label.eventId} className="flex flex-col gap-1 py-2 first:pt-0 last:pb-0">
              <div className="flex min-w-0 items-baseline gap-2">
                <span className="min-w-0 truncate text-sm font-medium">{label.title ?? t("labelProgress.recent.unknownMap")}</span>
                {label.version !== null && (
                  <span className="text-muted-foreground min-w-0 shrink truncate text-xs">{label.version}</span>
                )}
                <time
                  dateTime={label.at}
                  title={formatDateTime(label.at, i18n.language)}
                  className="text-muted-foreground ml-auto shrink-0 text-xs"
                >
                  {formatRelative(label.at, nowMs, i18n.language)}
                </time>
              </div>
              <div className="flex flex-wrap gap-1">
                {label.noPattern ? (
                  <Badge variant="outline">{t("labelProgress.recent.noPattern")}</Badge>
                ) : (
                  label.patterns.map((id) => (
                    <Badge key={id} variant="secondary" className="capitalize">
                      {patternName(id)}
                    </Badge>
                  ))
                )}
              </div>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
