import { useQuery } from "@tanstack/react-query";
import { ChartColumnBig } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { labelTaxonomyQuery } from "@/features/label";
import type { PatternDefDto } from "@/ipc/bindings";
import { PageHeader } from "@/shared/ui/page-header";
import { ActivityChart } from "./components/ActivityChart";
import { AxisBars } from "./components/AxisBars";
import { RankingCard } from "./components/RankingCard";
import { RecentLabels } from "./components/RecentLabels";
import { SessionList, type SessionThumbnailParams } from "./components/SessionList";
import { StatCards } from "./components/StatCards";
import { axisBars } from "./model";
import { labelProgressQuery, localUtcOffsetMin, sessionPlaysQuery } from "./queries";

export interface LabelProgressParams {
  /** The activity title's span until the series arrives; the service sends the real one. */
  activityDays: number;
  /** How often "played 5 minutes ago" and the local day are re-read while the page stays open. */
  clockTickMs: number;
  thumbnail: SessionThumbnailParams;
}

export const LABEL_PROGRESS_PARAMS: LabelProgressParams = {
  activityDays: 30,
  clockTickMs: 60_000,
  thumbnail: { rootMargin: "200px" },
};

export interface LabelProgressPageProps {
  keymode: number;
  params?: LabelProgressParams;
  now?: () => number;
}

const NO_TAXONOMY: readonly PatternDefDto[] = [];
const MS_PER_MIN = 60_000;
const MS_PER_DAY = 86_400_000;

function useClock(now: () => number, tickMs: number): number {
  const [nowMs, setNowMs] = useState(now);
  useEffect(() => {
    const id = setInterval(() => {
      setNowMs(now());
    }, tickMs);
    return () => {
      clearInterval(id);
    };
  }, [now, tickMs]);
  return nowMs;
}

export function LabelProgressPage({ keymode, params = LABEL_PROGRESS_PARAMS, now = Date.now }: LabelProgressPageProps) {
  const { t } = useTranslation();
  // Fixed for the page's life, so a DST change mid-visit does not fork the query key.
  const [utcOffsetMin] = useState(localUtcOffsetMin);
  const progress = useQuery(labelProgressQuery(keymode, utcOffsetMin));
  const session = useQuery(sessionPlaysQuery(keymode));
  const taxonomy = useQuery(labelTaxonomyQuery(keymode));
  const nowMs = useClock(now, params.clockTickMs);

  // The service cuts "today" when asked, so a page left open past midnight asks again.
  const localDay = Math.floor((nowMs + utcOffsetMin * MS_PER_MIN) / MS_PER_DAY);
  const shownDay = useRef(localDay);
  const { refetch: refetchProgress } = progress;
  useEffect(() => {
    if (shownDay.current !== localDay) {
      shownDay.current = localDay;
      void refetchProgress();
    }
  }, [localDay, refetchProgress]);

  const progressUnavailable = progress.isError && progress.data === undefined;
  const taxonomyFailure = taxonomy.isError
    ? {
        error: taxonomy.error,
        retry: () => {
          void taxonomy.refetch();
        },
      }
    : undefined;
  const families =
    progress.data === undefined || taxonomy.data === undefined
      ? undefined
      : axisBars(taxonomy.data, progress.data.perAxis, progress.data.perPattern);
  return (
    <>
      <PageHeader icon={ChartColumnBig} title={t("labelProgress.title")} description={t("labelProgress.subtitle")} />
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-6 px-6 py-6">
        <StatCards progress={progress} />
        <SessionList
          keymode={keymode}
          session={session}
          taxonomy={taxonomy.data ?? NO_TAXONOMY}
          taxonomyFailure={taxonomyFailure}
          nowMs={nowMs}
          thumbnail={params.thumbnail}
        />
        <div className="grid gap-6 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
          <AxisBars
            families={families}
            noPattern={progress.data?.goldNoPattern}
            unavailable={progressUnavailable}
            taxonomyFailure={taxonomyFailure}
          />
          <div className="flex min-w-0 flex-col gap-6">
            <ActivityChart perDay={progress.data?.perDay} days={params.activityDays} unavailable={progressUnavailable} />
            <RecentLabels recent={progress.data?.recent} nowMs={nowMs} unavailable={progressUnavailable} />
          </div>
        </div>
        <RankingCard />
      </div>
    </>
  );
}
