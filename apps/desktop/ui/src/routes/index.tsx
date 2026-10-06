import { useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import {
  CircleCheck,
  CircleSlash,
  CircleX,
  FolderOpen,
  History,
  House,
  LoaderCircle,
  type LucideIcon,
  RefreshCw,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { setupStatusQuery } from "@/features/setup";
import type { JobDto, JobStatusDto, SyncSummaryDto } from "@/ipc/bindings";
import { formatDateTime, formatNumber } from "@/shared/format";
import { Badge } from "@/shared/ui/badge";
import { buttonVariants } from "@/shared/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/shared/ui/card";
import { PageHeader } from "@/shared/ui/page-header";

export const Route = createFileRoute("/")({
  loader: ({ context }) => context.queryClient.query(setupStatusQuery()),
  component: HomePage,
});

const STATUS_ICON: Record<JobStatusDto, { icon: LucideIcon; className: string }> = {
  ok: { icon: CircleCheck, className: "text-success" },
  failed: { icon: CircleX, className: "text-destructive" },
  cancelled: { icon: CircleSlash, className: "text-muted-foreground" },
  queued: { icon: LoaderCircle, className: "text-osu-blue" },
  running: { icon: LoaderCircle, className: "text-osu-blue" },
};

const SYNC_STATS: readonly { key: "playsNew" | "playsExisting" | "replaysLinked"; className: string }[] = [
  { key: "playsNew", className: "text-osu-pink" },
  { key: "playsExisting", className: "text-foreground" },
  { key: "replaysLinked", className: "text-osu-blue" },
];

function SyncStats({ counters }: { counters: SyncSummaryDto }) {
  const { t, i18n } = useTranslation();
  return (
    <div className="grid grid-cols-3 gap-3">
      {SYNC_STATS.map(({ key, className }) => (
        <div key={key} className="bg-surface-raised flex flex-col gap-1 rounded-lg p-3">
          <span className={`font-display tabular text-2xl font-bold ${className}`}>
            {formatNumber(counters[key], i18n.language)}
          </span>
          <span className="text-muted-foreground text-xs tracking-wide uppercase">{t(`common.home.stats.${key}`)}</span>
        </div>
      ))}
    </div>
  );
}

function LastSync({ job }: { job: JobDto | null }) {
  const { t, i18n } = useTranslation();
  if (job === null) {
    return (
      <div className="text-muted-foreground flex flex-col items-center gap-2 py-4 text-center">
        <History className="size-6" aria-hidden="true" />
        <p>{t("common.home.neverSynced")}</p>
      </div>
    );
  }
  const when = job.ended ?? job.started;
  const { icon: StatusIcon, className } = STATUS_ICON[job.status];
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="outline" className="h-6 gap-1.5 px-2.5">
          <StatusIcon className={className} aria-hidden="true" />
          {t(`common.home.jobStatus.${job.status}`)}
        </Badge>
        {when !== null && <span className="text-muted-foreground text-sm">{formatDateTime(when, i18n.language)}</span>}
      </div>
      {job.summary?.kind === "sync_plays" && <SyncStats counters={job.summary.counters} />}
    </div>
  );
}

function HomePage() {
  const { t } = useTranslation();
  const { data: status } = useSuspenseQuery(setupStatusQuery());
  return (
    <>
      <PageHeader icon={House} title={t("common.home.title")} description={t("common.home.subtitle")} />
      <div className="mx-auto grid w-full max-w-5xl gap-4 px-6 py-6 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <FolderOpen className="text-primary size-4" aria-hidden="true" />
              {t("common.home.install")}
            </CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col items-start gap-3">
            {status.install === null ? (
              <p className="text-muted-foreground">{t("common.home.noInstall")}</p>
            ) : (
              <>
                <code className="bg-muted self-stretch rounded-md px-3 py-2 font-mono text-sm break-all">
                  {status.install.rootPath}
                </code>
                {status.install.osuDbVersion !== null && (
                  <Badge variant="secondary" className="tabular">
                    {t("common.home.dbVersion", { version: status.install.osuDbVersion })}
                  </Badge>
                )}
              </>
            )}
            <Link to="/setup" className={buttonVariants({ variant: "outline" })}>
              {t("common.home.changeInstall")}
            </Link>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <RefreshCw className="text-primary size-4" aria-hidden="true" />
              {t("common.home.lastSync")}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <LastSync job={status.lastSync} />
          </CardContent>
        </Card>
      </div>
    </>
  );
}
