import { useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { setupStatusQuery } from "@/features/setup";
import type { JobDto } from "@/ipc/bindings";
import { formatDateTime } from "@/shared/format";
import { buttonVariants } from "@/shared/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/shared/ui/card";

export const Route = createFileRoute("/")({
  loader: ({ context }) => context.queryClient.query(setupStatusQuery()),
  component: HomePage,
});

function LastSync({ job }: { job: JobDto | null }) {
  const { t, i18n } = useTranslation();
  if (job === null) {
    return <p className="text-muted-foreground">{t("common.home.neverSynced")}</p>;
  }
  const when = job.ended ?? job.started;
  return (
    <div className="flex flex-col gap-1">
      <p>
        {t(`common.home.jobStatus.${job.status}`)}
        {when !== null && <span className="text-muted-foreground"> · {formatDateTime(when, i18n.language)}</span>}
      </p>
      {job.summary?.kind === "sync_plays" && (
        <p className="text-muted-foreground text-sm">
          {t("common.home.syncSummary", {
            playsNew: job.summary.counters.playsNew,
            playsExisting: job.summary.counters.playsExisting,
            replaysLinked: job.summary.counters.replaysLinked,
          })}
        </p>
      )}
    </div>
  );
}

function HomePage() {
  const { t } = useTranslation();
  const { data: status } = useSuspenseQuery(setupStatusQuery());
  return (
    <div className="flex max-w-3xl flex-col gap-4 p-6">
      <h1 className="text-2xl font-semibold">{t("common.home.title")}</h1>
      <Card>
        <CardHeader>
          <CardTitle>{t("common.home.install")}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {status.install === null ? (
            <p className="text-muted-foreground">{t("common.home.noInstall")}</p>
          ) : (
            <code className="text-sm">{status.install.rootPath}</code>
          )}
          <Link to="/setup" className={buttonVariants({ variant: "outline", className: "self-start" })}>
            {t("common.home.changeInstall")}
          </Link>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>{t("common.home.lastSync")}</CardTitle>
        </CardHeader>
        <CardContent>
          <LastSync job={status.lastSync} />
        </CardContent>
      </Card>
    </div>
  );
}
