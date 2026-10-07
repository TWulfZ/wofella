import { useMutation } from "@tanstack/react-query";
import { ListChecks, LoaderCircle, PanelRightClose } from "lucide-react";
import { type FocusEvent, useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "cn";
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
import { JOB_TRAY_PARAMS, type JobTrayParams } from "./trayParams";

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

/**
 * Folds the panel away once the last job finished: only on that transition, so a panel the viewer opens to read
 * results stays put, and never while focus or the pointer is inside it.
 */
function useAutoCollapse(running: number, open: boolean, delayMs: number) {
  const panelRef = useRef<HTMLElement>(null);
  const pointerInside = useRef(false);
  const [prevRunning, setPrevRunning] = useState(running);
  const [armed, setArmed] = useState(false);
  const [retry, setRetry] = useState(0);

  if (running !== prevRunning) {
    setPrevRunning(running);
    setArmed(open && prevRunning > 0 && running === 0);
  }
  if (!open && armed) {
    setArmed(false);
  }

  useEffect(() => {
    if (!armed) {
      return undefined;
    }
    const timer = setTimeout(() => {
      // Read at fire time: a focused control removed from the DOM (Cancel turning into Dismiss) fires no blur.
      if (pointerInside.current || panelRef.current?.contains(document.activeElement) === true) {
        return;
      }
      setArmed(false);
      jobTrayStore.getState().setOpen(false);
    }, delayMs);
    return () => {
      clearTimeout(timer);
    };
  }, [armed, retry, delayMs]);

  const release = () => {
    if (armed) {
      setRetry((n) => n + 1);
    }
  };

  return {
    panelRef,
    panelHandlers: {
      onPointerEnter: () => {
        pointerInside.current = true;
      },
      onPointerLeave: () => {
        pointerInside.current = false;
        release();
      },
      onBlur: (event: FocusEvent<HTMLElement>) => {
        if (!(event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget))) {
          release();
        }
      },
    },
  };
}

export function JobTray({ params = JOB_TRAY_PARAMS }: { params?: JobTrayParams }) {
  const { t } = useTranslation();
  const jobs = useHydratedJobs();
  const open = useJobTray((s) => s.open);
  const panelId = useId();
  const tabRef = useRef<HTMLButtonElement>(null);
  const hideRef = useRef<HTMLButtonElement>(null);
  // Only the viewer's own toggle moves focus; a job opening the tray must not pull focus off the Label screen.
  const focusOnToggle = useRef(false);

  const ordered = [...jobs.values()].sort(trayOrder);
  const running = ordered.filter((j) => !isFinished(j.status)).length;
  // A sync that finished "ok" with failed items still failed some work; the tab must not hide it once the panel folds.
  const failed = ordered.filter((j) => j.status === "failed" || j.failedItems > 0).length;
  const { panelRef, panelHandlers } = useAutoCollapse(running, open, params.autoCollapseMs);

  useEffect(() => {
    if (focusOnToggle.current) {
      focusOnToggle.current = false;
      (open ? hideRef : tabRef).current?.focus();
    }
  }, [open]);

  const choose = (next: boolean) => {
    focusOnToggle.current = true;
    jobTrayStore.getState().chooseOpen(next);
  };

  if (!open) {
    const label = [
      t("jobs.show"),
      running > 0 ? t("jobs.running", { count: running }) : null,
      failed > 0 ? t("jobs.failed", { count: failed }) : null,
    ]
      .filter((part) => part !== null)
      .join(", ");
    const Icon = running > 0 ? LoaderCircle : ListChecks;
    return (
      <button
        ref={tabRef}
        type="button"
        aria-expanded={false}
        aria-label={label}
        className="bg-surface-raised ring-border hover:bg-accent focus-visible:ring-ring fixed top-1/2 right-0 z-30 flex w-(--job-tray-tab-w) -translate-y-1/2 cursor-pointer flex-col items-center gap-2 rounded-l-lg py-3 text-xs font-medium shadow-lg shadow-black/30 ring-1 outline-none focus-visible:ring-2 motion-safe:transition-colors motion-safe:duration-200"
        onClick={() => {
          choose(true);
        }}
      >
        <Icon className={cn("text-primary size-4", running > 0 && "motion-safe:animate-spin")} aria-hidden="true" />
        <span className="[writing-mode:vertical-rl]" aria-hidden="true">
          {t("jobs.show")}
        </span>
        {running > 0 && (
          <span
            aria-hidden="true"
            className="bg-primary text-primary-foreground tabular inline-flex size-5 items-center justify-center rounded-full text-[0.625rem] font-semibold"
          >
            {running}
          </span>
        )}
        {failed > 0 && (
          <span
            aria-hidden="true"
            className="bg-destructive/20 text-destructive ring-destructive/60 tabular inline-flex size-5 items-center justify-center rounded-full text-[0.625rem] font-semibold ring-1"
          >
            {failed}
          </span>
        )}
      </button>
    );
  }

  return (
    <section
      ref={panelRef}
      id={panelId}
      aria-label={t("jobs.title")}
      className="bg-card text-card-foreground ring-border fixed right-4 bottom-4 z-30 flex max-h-[60vh] w-96 max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-xl shadow-2xl shadow-black/40 ring-1"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          // Screens listen for Esc on window; this one belongs to the tray.
          event.stopPropagation();
          choose(false);
        }
      }}
      {...panelHandlers}
    >
      <div className="bg-header/60 flex items-center justify-between gap-2 border-b px-4 py-2.5">
        <h2 className="font-display flex items-center gap-2 font-semibold">
          <ListChecks className="text-primary size-4" aria-hidden="true" />
          {t("jobs.title")}
        </h2>
        <Button
          ref={hideRef}
          variant="ghost"
          size="xs"
          aria-expanded={true}
          aria-controls={panelId}
          onClick={() => {
            choose(false);
          }}
        >
          <PanelRightClose aria-hidden="true" />
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
