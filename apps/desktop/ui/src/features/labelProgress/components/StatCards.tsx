import type { UseQueryResult } from "@tanstack/react-query";
import { CalendarCheck, Gamepad2, type LucideIcon, Trophy } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { LabelProgressDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { formatNumber } from "@/shared/format";
import { Button } from "@/shared/ui/button";
import { type GoldOrigins as Origins, goldOrigins, todayCounts } from "../model";

interface StatCardProps {
  icon: LucideIcon;
  caption: string;
  value: number | undefined;
  hint: string;
  tone: string;
}

function StatCard({ icon: Icon, caption, value, hint, tone }: StatCardProps) {
  const { i18n } = useTranslation();
  const captionId = useId();
  return (
    <div role="group" aria-labelledby={captionId} className="bg-card ring-border flex items-center gap-4 rounded-xl p-4 shadow-lg shadow-black/20 ring-1">
      <div className={`grid size-11 shrink-0 place-items-center rounded-xl ${tone}`}>
        <Icon aria-hidden="true" className="size-5" />
      </div>
      <div className="flex min-w-0 flex-col">
        <span id={captionId} className="text-muted-foreground text-sm">
          {caption}
        </span>
        <span className="font-display text-3xl leading-tight font-bold tabular-nums">
          {value === undefined ? "–" : formatNumber(value, i18n.language)}
        </span>
        <span className="text-muted-foreground truncate text-xs">{hint}</span>
      </div>
    </div>
  );
}

const ORIGIN_TONES: Readonly<Record<keyof Origins, string>> = {
  blind: "bg-osu-lime",
  chosen: "bg-osu-blue",
  unknown: "bg-muted-foreground/60",
};

const ORIGIN_ORDER: readonly (keyof Origins)[] = ["blind", "chosen", "unknown"];

function GoldOrigins({ origins, total }: { origins: Origins; total: number }) {
  const { t } = useTranslation();
  const captionId = useId();
  return (
    <div role="group" aria-labelledby={captionId} className="bg-card ring-border flex flex-col gap-2 rounded-xl p-4 shadow-lg shadow-black/20 ring-1">
      <span id={captionId} className="text-muted-foreground text-sm">
        {t("labelProgress.stats.origins")}
      </span>
      {/* The list below carries the numbers; the bar only shows the proportions. */}
      <div aria-hidden="true" className="bg-muted flex h-2 overflow-hidden rounded-full">
        {total > 0 &&
          ORIGIN_ORDER.map((origin) => (
            <div key={origin} className={ORIGIN_TONES[origin]} style={{ width: `${String((origins[origin] / total) * 100)}%` }} />
          ))}
      </div>
      <ul className="flex flex-wrap gap-x-4 gap-y-1 text-sm tabular-nums">
        {ORIGIN_ORDER.map((origin) => (
          <li key={origin} className="flex items-center gap-1.5">
            <span aria-hidden="true" className={`size-2 rounded-full ${ORIGIN_TONES[origin]}`} />
            {t(`labelProgress.stats.${origin}`, { count: origins[origin] })}
          </li>
        ))}
      </ul>
      <p className="text-muted-foreground text-xs">{t("labelProgress.stats.originsExplained")}</p>
    </div>
  );
}

export function StatCards({ progress }: { progress: UseQueryResult<LabelProgressDto> }) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const headingId = useId();
  const data = progress.data;
  const today = data === undefined ? undefined : todayCounts(data.perDay);
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-3">
      <h2 id={headingId} className="sr-only">
        {t("labelProgress.stats.label")}
      </h2>
      {progress.isError && (
        <div role="alert" className="bg-destructive/10 text-destructive flex flex-wrap items-center gap-3 rounded-lg px-3 py-2 text-sm">
          <span>{errorText(progress.error)}</span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              void progress.refetch();
            }}
          >
            {t("common.retry")}
          </Button>
        </div>
      )}
      <div className="grid gap-3 sm:grid-cols-3">
        <StatCard
          icon={Trophy}
          caption={t("labelProgress.stats.gold")}
          value={data?.goldTotal}
          hint={t("labelProgress.stats.goldHint")}
          tone="bg-osu-yellow/15 text-osu-yellow"
        />
        <StatCard
          icon={Gamepad2}
          caption={t("labelProgress.stats.session")}
          value={data?.sessionLabels}
          hint={t("labelProgress.stats.sessionHint")}
          tone="bg-osu-blue/15 text-osu-blue"
        />
        <StatCard
          icon={CalendarCheck}
          caption={t("labelProgress.stats.today")}
          value={today === undefined ? undefined : today.gold + today.session}
          hint={today === undefined ? "" : t("labelProgress.stats.todayHint", { ...today })}
          tone="bg-primary/15 text-primary"
        />
      </div>
      {data !== undefined && <GoldOrigins origins={goldOrigins(data)} total={data.goldTotal} />}
    </section>
  );
}
