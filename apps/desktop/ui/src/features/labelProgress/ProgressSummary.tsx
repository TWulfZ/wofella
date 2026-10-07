import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ArrowRight } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { useErrorText } from "@/ipc/errorText";
import { formatNumber } from "@/shared/format";
import { buttonVariants } from "@/shared/ui/button";
import { sessionCountsText } from "./countText";
import { pendingMaps, sessionMapCounts, todayCounts } from "./model";
import { labelProgressQuery, localUtcOffsetMin, sessionPlaysQuery } from "./queries";

export const PROGRESS_SUMMARY_PARAMS = {
  /** Pending maps named by title; the rest are counted. */
  pendingShown: 3,
} as const;

export interface ProgressSummaryProps {
  keymode: number;
  pendingShown?: number;
}

/** The Label screen's counters popover: a glance at the progress page, which holds the detail. */
export function ProgressSummary({ keymode, pendingShown = PROGRESS_SUMMARY_PARAMS.pendingShown }: ProgressSummaryProps) {
  const { t, i18n } = useTranslation();
  const errorText = useErrorText();
  const headingId = useId();
  const pendingId = useId();
  const [utcOffsetMin] = useState(localUtcOffsetMin);
  const progress = useQuery(labelProgressQuery(keymode, utcOffsetMin));
  const session = useQuery(sessionPlaysQuery(keymode));
  const today = progress.data === undefined ? undefined : todayCounts(progress.data.perDay);
  const counts = session.data === undefined ? undefined : sessionMapCounts(session.data.plays);
  const pending = session.data === undefined ? [] : pendingMaps(session.data.plays);
  const error = progress.error ?? session.error;
  const number = (value: number | undefined): string => (value === undefined ? "–" : formatNumber(value, i18n.language));
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-3 text-sm">
      <h2 id={headingId} className="font-display text-sm font-semibold">
        {t("labelProgress.summary.title")}
      </h2>
      {error !== null && (
        <p role="alert" className="text-destructive text-xs">
          {errorText(error)}
        </p>
      )}
      <dl className="grid grid-cols-[1fr_auto] gap-x-4 gap-y-1.5">
        <dt className="text-muted-foreground">{t("labelProgress.summary.gold")}</dt>
        <dd className="text-right font-semibold tabular-nums">{number(progress.data?.goldTotal)}</dd>
        <dt className="text-muted-foreground">{t("labelProgress.summary.today")}</dt>
        <dd className="text-right font-semibold tabular-nums">
          {number(today === undefined ? undefined : today.gold + today.session)}
        </dd>
        <dt className="text-muted-foreground">{t("labelProgress.summary.session")}</dt>
        <dd className="text-right font-semibold tabular-nums">
          {counts === undefined ? "–" : sessionCountsText(t, "labelProgress.summary.sessionValue", counts)}
        </dd>
      </dl>
      {pending.length > 0 && (
        <div className="flex flex-col gap-1 border-t pt-2">
          <span id={pendingId} className="text-muted-foreground text-xs">
            {t("labelProgress.summary.pendingMaps")}
          </span>
          <ul aria-labelledby={pendingId} className="flex flex-col gap-0.5">
            {pending.slice(0, pendingShown).map((play) => (
              <li key={play.md5} className="truncate">
                {play.title}
              </li>
            ))}
          </ul>
          {pending.length > pendingShown && (
            <span className="text-muted-foreground text-xs">
              {t("labelProgress.summary.more", { count: pending.length - pendingShown })}
            </span>
          )}
        </div>
      )}
      <Link
        to="/label/progress"
        search={(prev) => prev}
        className={buttonVariants({ variant: "outline", size: "sm", className: "self-start" })}
      >
        {t("labelProgress.summary.link")}
        <ArrowRight aria-hidden="true" />
      </Link>
    </section>
  );
}
