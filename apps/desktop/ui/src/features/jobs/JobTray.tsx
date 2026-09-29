import { useMutation } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { commands, type JobId } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { localizeIpcError, useErrorText } from "@/ipc/errorText";
import { formatEta } from "@/shared/format";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { Progress } from "@/shared/ui/progress";
import { useHydratedJobs } from "./hooks";
import { isFinished, type TrayJob } from "./store";
import { jobTrayStore, useJobTray } from "./tray";

const PERCENT = 100;

const STATUS_VARIANT = {
  queued: "secondary",
  running: "secondary",
  ok: "default",
  failed: "destructive",
  cancelled: "outline",
} as const;

function JobRow({ job }: { job: TrayJob }) {
  const { t, i18n } = useTranslation();
  const errorText = useErrorText();
  const cancel = useMutation({ mutationFn: (id: JobId) => call(commands.jobsCancel(id)) });
  const finished = isFinished(job.status);
  const hasTotal = job.total > 0;
  return (
    <li className="flex flex-col gap-1.5 rounded-md border p-2 text-sm">
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium">{t(`jobs.kind.${job.kind ?? "unknown"}`)}</span>
        <Badge variant={STATUS_VARIANT[job.status]}>{t(`jobs.status.${job.status}`)}</Badge>
      </div>
      {!finished && (
        <>
          <Progress value={hasTotal ? (job.done / job.total) * PERCENT : null} />
          <div className="text-muted-foreground flex justify-between gap-2 text-xs">
            <span>
              {job.stage !== null && t(`jobs.stage.${job.stage}`)}
              {hasTotal && <> · {t("jobs.progress", { done: job.done, total: job.total })}</>}
            </span>
            {job.etaMs !== null && <span>{t("jobs.eta", { eta: formatEta(job.etaMs) })}</span>}
          </div>
        </>
      )}
      {job.failedItems > 0 && <span className="text-destructive text-xs">{t("jobs.failedItems", { count: job.failedItems })}</span>}
      {job.error !== null && job.status === "failed" && (
        <span className="text-muted-foreground text-xs">
          {localizeIpcError(
            { ...job.error, args: {}, details: null, retryable: false },
            { t: (key, options) => t(key, options ?? {}), exists: (key) => i18n.exists(key) },
          )}
        </span>
      )}
      {cancel.isError && (
        <span role="alert" className="text-destructive text-xs">
          {errorText(cancel.error)}
        </span>
      )}
      <div className="flex justify-end">
        {finished ? (
          <Button
            variant="ghost"
            size="xs"
            onClick={() => {
              jobTrayStore.getState().dismiss(job.id);
            }}
          >
            {t("jobs.dismiss")}
          </Button>
        ) : (
          <Button
            variant="outline"
            size="xs"
            disabled={cancel.isPending}
            onClick={() => {
              cancel.mutate(job.id);
            }}
          >
            {t("jobs.cancel")}
          </Button>
        )}
      </div>
    </li>
  );
}

// Running jobs first, then finished ones newest first.
function trayOrder(a: TrayJob, b: TrayJob): number {
  if (a.finishedSeq === null || b.finishedSeq === null) {
    return (a.finishedSeq === null ? 0 : 1) - (b.finishedSeq === null ? 0 : 1);
  }
  return b.finishedSeq - a.finishedSeq;
}

export function JobTray() {
  const { t } = useTranslation();
  const jobs = useHydratedJobs();
  const open = useJobTray((s) => s.open);

  const ordered = [...jobs.values()].sort(trayOrder);
  const running = ordered.filter((j) => !isFinished(j.status)).length;

  if (!open) {
    return (
      <Button
        variant="outline"
        className="fixed right-4 bottom-4 shadow-md"
        onClick={() => {
          jobTrayStore.getState().setOpen(true);
        }}
      >
        {t("jobs.show")}
        {running > 0 && <Badge variant="secondary">{t("jobs.running", { count: running })}</Badge>}
      </Button>
    );
  }

  return (
    <section
      aria-label={t("jobs.title")}
      className="bg-background fixed right-4 bottom-4 flex max-h-[60vh] w-80 flex-col gap-2 rounded-lg border p-3 shadow-lg"
    >
      <div className="flex items-center justify-between">
        <h2 className="font-semibold">{t("jobs.title")}</h2>
        <Button
          variant="ghost"
          size="xs"
          onClick={() => {
            jobTrayStore.getState().setOpen(false);
          }}
        >
          {t("jobs.hide")}
        </Button>
      </div>
      {ordered.length === 0 && <p className="text-muted-foreground text-sm">{t("jobs.empty")}</p>}
      <ul aria-label={t("jobs.title")} className="flex flex-col gap-2 overflow-y-auto">
        {ordered.map((job) => (
          <JobRow key={job.id} job={job} />
        ))}
      </ul>
    </section>
  );
}
