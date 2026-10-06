import { useMutation } from "@tanstack/react-query";
import { ListChecks, LoaderCircle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { commands, type JobId } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { localizeIpcError, useErrorText } from "@/ipc/errorText";
import { formatEta } from "@/shared/format";
import { Button } from "@/shared/ui/button";
import { Progress } from "@/shared/ui/progress";
import { useHydratedJobs } from "./hooks";
import { JobStatusIcon } from "./JobStatusIcon";
import { isFinished, type TrayJob } from "./store";
import { jobTrayStore, useJobTray } from "./tray";

const PERCENT = 100;

function JobRow({ job }: { job: TrayJob }) {
  const { t, i18n } = useTranslation();
  const errorText = useErrorText();
  const cancel = useMutation({ mutationFn: (id: JobId) => call(commands.jobsCancel(id)) });
  const finished = isFinished(job.status);
  const hasTotal = job.total > 0;
  return (
    <li className="bg-surface-raised/60 ring-border/60 flex flex-col gap-2 rounded-lg p-3 text-sm ring-1">
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium">{t(`jobs.kind.${job.kind ?? "unknown"}`)}</span>
        <span className="text-muted-foreground flex items-center gap-1.5 text-xs font-medium">
          <JobStatusIcon status={job.status} className="size-3.5" />
          {t(`jobs.status.${job.status}`)}
        </span>
      </div>
      {!finished && (
        <>
          {/* A queued job has done nothing yet; any bar would claim progress it has not made. */}
          {job.status !== "queued" && <Progress value={hasTotal ? (job.done / job.total) * PERCENT : null} />}
          <div className="text-muted-foreground tabular flex justify-between gap-2 text-xs">
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
      <button
        type="button"
        className="bg-surface-raised ring-border hover:bg-accent focus-visible:ring-ring fixed right-4 bottom-4 z-30 inline-flex h-9 cursor-pointer items-center gap-2 rounded-full pr-2 pl-3.5 text-sm font-medium shadow-lg shadow-black/30 ring-1 transition-colors duration-200 outline-none focus-visible:ring-2"
        onClick={() => {
          jobTrayStore.getState().setOpen(true);
        }}
      >
        <ListChecks className="text-primary size-4" aria-hidden="true" />
        {t("jobs.show")}
        {running > 0 && (
          <span className="bg-primary text-primary-foreground tabular inline-flex h-5 items-center gap-1 rounded-full px-2 text-xs font-semibold">
            <LoaderCircle className="size-3 motion-safe:animate-spin" aria-hidden="true" />
            {t("jobs.running", { count: running })}
          </span>
        )}
      </button>
    );
  }

  return (
    <section
      aria-label={t("jobs.title")}
      className="bg-card text-card-foreground ring-border fixed right-4 bottom-4 z-30 flex max-h-[60vh] w-96 flex-col overflow-hidden rounded-xl shadow-2xl shadow-black/40 ring-1"
    >
      <div className="bg-header/60 flex items-center justify-between gap-2 border-b px-4 py-2.5">
        <h2 className="font-display flex items-center gap-2 font-semibold">
          <ListChecks className="text-primary size-4" aria-hidden="true" />
          {t("jobs.title")}
        </h2>
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
      {ordered.length === 0 && <p className="text-muted-foreground px-4 py-3 text-sm">{t("jobs.empty")}</p>}
      <ul aria-label={t("jobs.title")} className="flex flex-col gap-2 overflow-y-auto p-3 empty:hidden">
        {ordered.map((job) => (
          <JobRow key={job.id} job={job} />
        ))}
      </ul>
    </section>
  );
}
