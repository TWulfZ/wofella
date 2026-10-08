import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleAlert, CircleCheck, LoaderCircle, ShieldCheck } from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { isFinished, jobKeys, jobsListQuery, type TrayJob, useTrayJob } from "@/features/jobs";
import { commands, type JobDto, type JobId, type RateCopyPlanDto, type RateCopySummaryDto } from "@/ipc/bindings";
import { call, type IpcError, toIpcError } from "@/ipc/client";
import { localizeIpcError } from "@/ipc/errorText";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/shared/ui/dialog";
import { Progress } from "@/shared/ui/progress";
import { rateCopyPlanQuery } from "./queries";

export interface RateCopyTarget {
  md5: string;
  rateMilli: number;
  /** "Artist - Title [Version]" of the source chart. */
  chartLabel: string;
}

export interface RateCopyDialogProps {
  /** Null keeps the dialog closed. */
  target: RateCopyTarget | null;
  onOpenChange: (open: boolean) => void;
}

const REFUSED_KEY = "rate_copy.error.refused";
const PERCENT = 100;

export function useRateText() {
  const { t, i18n } = useTranslation();
  const rate = new Intl.NumberFormat(i18n.language, { minimumFractionDigits: 2, maximumFractionDigits: 3 });
  return (rateMilli: number) => t("rateCopy.rateValue", { rate: rate.format(rateMilli / 1000) });
}

function useRefusalText() {
  const { t, i18n } = useTranslation();
  return (code: string) => {
    const key = `rateCopy.refusal.${code}`;
    return t(i18n.exists(key) ? key : "rateCopy.refusal.unknown");
  };
}

/** Localizes an IpcError, naming the refusal itself when confirm reports one instead of its raw id. */
function useRateCopyErrorText() {
  const { t, i18n } = useTranslation();
  const refusalText = useRefusalText();
  return (error: IpcError) => {
    const refusal = error.args["refusal"];
    if (error.messageKey === REFUSED_KEY && refusal !== undefined) {
      return refusalText(refusal);
    }
    return localizeIpcError(error, { t: (key, options) => t(key, options ?? {}), exists: (key) => i18n.exists(key) });
  };
}

function Field({ term, children }: { term: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[8.5rem_minmax(0,1fr)] items-baseline gap-3">
      <dt className="text-muted-foreground text-xs font-medium tracking-wide uppercase">{term}</dt>
      <dd className="min-w-0 break-words">{children}</dd>
    </div>
  );
}

function FileName({ children }: { children: string }) {
  return <span className="font-mono text-[0.8rem] break-all">{children}</span>;
}

function Plan({ plan, target }: { plan: RateCopyPlanDto; target: RateCopyTarget }) {
  const { t } = useTranslation();
  const rateText = useRateText();
  const writes = (plan.osuExists ? 0 : 1) + (plan.audioExists ? 0 : 1);
  return (
    <div className="flex flex-col gap-4">
      <dl className="flex flex-col gap-2.5">
        <Field term={t("rateCopy.field.chart")}>{target.chartLabel}</Field>
        <Field term={t("rateCopy.field.rate")}>
          <span className="font-display text-osu-purple text-base font-bold italic tabular-nums">{rateText(plan.rateMilli)}</span>
        </Field>
        <Field term={t("rateCopy.field.folder")}>
          <FileName>{plan.folder}</FileName>
        </Field>
        <Field term={t("rateCopy.field.osuFile")}>
          <span className="flex flex-col items-start gap-1">
            <FileName>{plan.osuFilename}</FileName>
            {plan.osuExists && <Badge className="bg-muted text-muted-foreground">{t("rateCopy.osu.exists")}</Badge>}
          </span>
        </Field>
        <Field term={t("rateCopy.field.version")}>{plan.version}</Field>
        <Field term={t("rateCopy.field.audio")}>
          <span className="flex flex-col items-start gap-1">
            <FileName>{plan.audioFilename}</FileName>
            <Badge className={plan.audioExists ? "bg-osu-blue/15 text-osu-blue" : "bg-osu-pink/15 text-osu-pink"}>
              {t(plan.audioExists ? "rateCopy.audio.reused" : "rateCopy.audio.willGenerate")}
            </Badge>
          </span>
        </Field>
      </dl>
      <p className="bg-osu-yellow/10 text-osu-yellow flex items-start gap-2 rounded-lg px-3 py-2 text-sm">
        <ShieldCheck aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
        <span>{writes === 0 ? t("rateCopy.writesNothing") : t("rateCopy.writes", { count: writes })}</span>
      </p>
    </div>
  );
}

function Refusal({ code }: { code: string }) {
  const { t } = useTranslation();
  const refusalText = useRefusalText();
  return (
    <div className="bg-destructive/10 flex items-start gap-3 rounded-lg px-3 py-3">
      <CircleAlert aria-hidden="true" className="text-destructive mt-0.5 size-5 shrink-0" />
      <div className="flex flex-col gap-1">
        <p className="text-destructive font-semibold">{t("rateCopy.refused")}</p>
        <p>{refusalText(code)}</p>
      </div>
    </div>
  );
}

function Running({ job, target }: { job: TrayJob | undefined; target: RateCopyTarget }) {
  const { t } = useTranslation();
  const rateText = useRateText();
  const started = job !== undefined && job.status !== "queued" && job.stage !== null;
  const hasTotal = job !== undefined && job.total > 0;
  return (
    <div role="status" className="flex flex-col gap-3">
      <p className="flex items-center gap-2 font-semibold">
        <LoaderCircle aria-hidden="true" className="text-primary size-4 motion-safe:animate-spin" />
        {t("rateCopy.progress.title")}
      </p>
      <p className="text-muted-foreground">
        {target.chartLabel} · {rateText(target.rateMilli)}
      </p>
      <Progress value={hasTotal ? (job.done / job.total) * PERCENT : null} />
      <p className="text-muted-foreground tabular flex justify-between gap-2 text-xs">
        <span>{started ? t(`jobs.stage.${job.stage}`) : t("rateCopy.progress.queued")}</span>
        {hasTotal && <span>{t("jobs.progress", { done: job.done, total: job.total })}</span>}
      </p>
    </div>
  );
}

function Success({ summary }: { summary: RateCopySummaryDto }) {
  const { t } = useTranslation();
  const audioState = summary.audioReused ? "reused" : summary.audioWritten ? "written" : "kept";
  const files = [
    { name: summary.osuFilename, state: summary.osuWritten ? "written" : "kept" },
    { name: summary.audioFilename, state: audioState },
  ];
  return (
    <div className="flex flex-col gap-4">
      <p className="bg-success/10 text-success flex items-start gap-2 rounded-lg px-3 py-2 font-medium">
        <CircleCheck aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
        <span>{t("rateCopy.done")}</span>
      </p>
      <ul className="flex flex-col gap-2">
        {files.map((file) => (
          <li key={file.name} className="bg-muted/30 flex flex-wrap items-center justify-between gap-2 rounded-md border px-3 py-2">
            <FileName>{file.name}</FileName>
            <Badge className={file.state === "written" ? "bg-success/15 text-success" : "bg-muted text-muted-foreground"}>
              {t(`rateCopy.file.${file.state}`)}
            </Badge>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Failure({ job }: { job: JobDto }) {
  const { t } = useTranslation();
  const errorText = useRateCopyErrorText();
  const cancelled = job.status === "cancelled";
  return (
    <div role="alert" className="bg-destructive/10 flex items-start gap-3 rounded-lg px-3 py-3">
      <CircleAlert aria-hidden="true" className="text-destructive mt-0.5 size-5 shrink-0" />
      <div className="flex flex-col gap-1">
        <p className="text-destructive font-semibold">{t(cancelled ? "rateCopy.cancelled" : "rateCopy.failed")}</p>
        {job.error !== null && <p>{errorText(toIpcError({ ...job.error, args: {}, details: null, retryable: false }))}</p>}
      </div>
    </div>
  );
}

/**
 * Follows the confirmed job: live progress comes from the tray's event feed, the outcome (summary or error) from
 * jobs_list, re-read once the job is seen finishing because JobFinished carries neither.
 */
function useRateCopyJob(jobId: JobId | null) {
  const queryClient = useQueryClient();
  const live = useTrayJob(jobId);
  const list = useQuery({ ...jobsListQuery(), enabled: jobId !== null });
  const finishedLive = live !== undefined && isFinished(live.status);
  useEffect(() => {
    if (finishedLive) {
      void queryClient.invalidateQueries({ queryKey: jobKeys.list() });
    }
  }, [finishedLive, queryClient]);
  const listed = list.data?.find((job) => job.id === jobId);
  return { live, outcome: listed !== undefined && isFinished(listed.status) ? listed : undefined };
}

function Flow({ target, onClose }: { target: RateCopyTarget; onClose: () => void }) {
  const { t } = useTranslation();
  const errorText = useRateCopyErrorText();
  const [jobId, setJobId] = useState<JobId | null>(null);
  const confirm = useMutation({
    mutationFn: (previewId: string) => call(commands.rateCopyConfirm(previewId)),
    onSuccess: setJobId,
  });
  const plan = useQuery({
    ...rateCopyPlanQuery(target.md5, target.rateMilli),
    // Once confirmed the preview is spent; re-planning would only mint ids nobody can use.
    enabled: jobId === null && !confirm.isPending,
  });
  const job = useRateCopyJob(jobId);

  let body: ReactNode;
  let canGenerate = false;
  if (jobId !== null) {
    const summary = job.outcome?.summary;
    if (job.outcome?.status === "ok" && summary?.kind === "rate_copy") {
      body = <Success summary={summary.counters} />;
    } else if (job.outcome !== undefined && job.outcome.status !== "ok") {
      body = <Failure job={job.outcome} />;
    } else {
      body = <Running job={job.live} target={target} />;
    }
  } else if (plan.isError) {
    body = (
      <p role="alert" className="text-destructive">
        {errorText(toIpcError(plan.error))}
      </p>
    );
  } else if (plan.data === undefined) {
    body = (
      <p role="status" className="text-muted-foreground flex items-center gap-2">
        <LoaderCircle aria-hidden="true" className="text-primary size-4 motion-safe:animate-spin" />
        {t("rateCopy.planning")}
      </p>
    );
  } else if (plan.data.refusal !== null) {
    body = <Refusal code={plan.data.refusal} />;
  } else {
    canGenerate = true;
    body = <Plan plan={plan.data} target={target} />;
  }

  const previewId = plan.data?.previewId;
  return (
    <>
      {body}
      {jobId === null && confirm.isError && (
        <p role="alert" className="bg-destructive/10 text-destructive rounded-lg px-3 py-2">
          {errorText(toIpcError(confirm.error))}
        </p>
      )}
      <DialogFooter>
        {canGenerate && previewId !== undefined ? (
          <>
            <Button type="button" variant="outline" onClick={onClose}>
              {t("rateCopy.actions.cancel")}
            </Button>
            <Button
              type="button"
              disabled={confirm.isPending}
              onClick={() => {
                confirm.mutate(previewId);
              }}
            >
              {t("rateCopy.actions.generate")}
            </Button>
          </>
        ) : (
          <Button type="button" variant="outline" onClick={onClose}>
            {t("rateCopy.actions.close")}
          </Button>
        )}
      </DialogFooter>
    </>
  );
}

/** Plan → confirm → job progress → "press F5" for one rate copy (ADR 0025). Closing never cancels a started job. */
export function RateCopyDialog({ target, onOpenChange }: RateCopyDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={target !== null} onOpenChange={onOpenChange}>
      {target !== null && (
        <DialogContent showCloseButton={false} className="max-h-[92dvh] gap-5 overflow-y-auto sm:max-w-xl">
          <DialogHeader>
            <DialogTitle className="font-display text-xl font-semibold">{t("rateCopy.title")}</DialogTitle>
            <DialogDescription>{t("rateCopy.description")}</DialogDescription>
          </DialogHeader>
          <Flow
            key={`${target.md5}-${String(target.rateMilli)}`}
            target={target}
            onClose={() => {
              onOpenChange(false);
            }}
          />
        </DialogContent>
      )}
    </Dialog>
  );
}
