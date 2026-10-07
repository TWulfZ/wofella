import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { DayCountDto } from "@/ipc/bindings";
import { activityTotals } from "../model";

function height(count: number, peak: number): string {
  return `${peak === 0 ? 0 : (count / peak) * 100}%`;
}

interface ActivityChartProps {
  perDay: readonly DayCountDto[] | undefined;
  days: number;
  unavailable: boolean;
}

export function ActivityChart({ perDay, days, unavailable }: ActivityChartProps) {
  const { t } = useTranslation();
  const headingId = useId();
  const shownDays = perDay?.length ?? days;
  const totals = perDay === undefined ? undefined : activityTotals(perDay);
  return (
    <section aria-labelledby={headingId} className="bg-card ring-border flex flex-col gap-3 rounded-xl p-5 shadow-lg shadow-black/20 ring-1">
      <div className="flex items-center justify-between gap-2">
        <h2 id={headingId} className="font-display text-base font-semibold">
          {t("labelProgress.activity.title", { days: shownDays })}
        </h2>
        <ul className="text-muted-foreground flex items-center gap-3 text-xs">
          <li className="flex items-center gap-1.5">
            <span aria-hidden="true" className="bg-osu-yellow size-2.5 rounded-sm" />
            {t("labelProgress.activity.gold")}
          </li>
          <li className="flex items-center gap-1.5">
            <span aria-hidden="true" className="bg-osu-blue size-2.5 rounded-sm" />
            {t("labelProgress.activity.session")}
          </li>
        </ul>
      </div>
      {perDay === undefined || totals === undefined ? (
        <p className="text-muted-foreground text-sm">{t(unavailable ? "labelProgress.unavailable" : "common.loading")}</p>
      ) : (
        <figure className="flex flex-col gap-2">
          {/* The bars repeat the summary below for sighted users; assistive tech reads the summary. */}
          <ol aria-hidden="true" className="bg-surface-raised flex h-24 items-end gap-0.5 rounded-lg p-1.5">
            {perDay.map((d) => (
              <li
                key={d.day}
                data-day={d.day}
                title={t("labelProgress.activity.day", d)}
                className="flex h-full flex-1 flex-col justify-end gap-px"
              >
                <span style={{ height: height(d.session, totals.peak) }} className="bg-osu-blue rounded-t-sm" />
                <span style={{ height: height(d.gold, totals.peak) }} className="bg-osu-yellow rounded-sm" />
              </li>
            ))}
          </ol>
          <figcaption className="text-muted-foreground text-xs">
            {totals.gold + totals.session === 0
              ? t("labelProgress.activity.empty", { days: shownDays })
              : t("labelProgress.activity.summary", {
                  days: shownDays,
                  gold: t("labelProgress.count.goldLabels", { count: totals.gold }),
                  session: t("labelProgress.count.sessionMaps", { count: totals.session }),
                  active: t("labelProgress.count.days", { count: totals.activeDays }),
                })}
          </figcaption>
        </figure>
      )}
    </section>
  );
}
