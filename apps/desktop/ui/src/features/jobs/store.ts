import { createStore, type StoreApi } from "zustand/vanilla";
import type {
  JobDto,
  JobErrorDto,
  JobFinishedDto,
  JobId,
  JobKindDto,
  JobProgressDto,
  JobStageDto,
  JobStatusDto,
} from "@/ipc/bindings";
import type { JobTrayPrefs } from "./trayParams";

// Spec 005 Behaviour "Job tray": finished jobs stay until dismissed or pushed out by this many newer ones.
export const FINISHED_CAP = 20;

const FINISHED_STATUSES: ReadonlySet<JobStatusDto> = new Set(["ok", "failed", "cancelled"]);

export function isFinished(status: JobStatusDto): boolean {
  return FINISHED_STATUSES.has(status);
}

export interface TrayJob {
  id: JobId;
  // Null when only a JobFinished was seen: that event carries no kind.
  kind: JobKindDto | null;
  status: JobStatusDto;
  stage: JobStageDto | null;
  done: number;
  total: number;
  etaMs: number | null;
  failedItems: number;
  error: JobErrorDto | null;
  // Monotonic finish order used for eviction; server timestamps and event arrival are merged into one sequence.
  finishedSeq: number | null;
}

export interface JobTrayState {
  jobs: ReadonlyMap<JobId, TrayJob>;
  open: boolean;
  applyProgress: (progress: JobProgressDto) => void;
  applyFinished: (finished: JobFinishedDto) => void;
  hydrate: (jobs: readonly JobDto[]) => void;
  dismiss: (id: JobId) => void;
  /** Transient: automatic opens and closes that must not overwrite what the viewer chose. */
  setOpen: (open: boolean) => void;
  /** The viewer's own toggle, remembered across launches. */
  chooseOpen: (open: boolean) => void;
  reset: () => void;
}

export type JobTrayStore = StoreApi<JobTrayState>;

function blank(id: JobId): TrayJob {
  return {
    id,
    kind: null,
    status: "queued",
    stage: null,
    done: 0,
    total: 0,
    etaMs: null,
    failedItems: 0,
    error: null,
    finishedSeq: null,
  };
}

function evictOldestFinished(jobs: Map<JobId, TrayJob>): void {
  const finished = [...jobs.values()]
    .filter((j) => j.finishedSeq !== null)
    .sort((a, b) => (a.finishedSeq ?? 0) - (b.finishedSeq ?? 0));
  for (const job of finished.slice(0, Math.max(0, finished.length - FINISHED_CAP))) {
    jobs.delete(job.id);
  }
}

const NO_PREFS: JobTrayPrefs = { read: () => null, write: () => undefined };

export function createJobTrayStore({ prefs = NO_PREFS }: { prefs?: JobTrayPrefs } = {}): JobTrayStore {
  // Dismissals, the finish counter and the remembered choice live outside React state: they only steer future updates.
  let dismissed = new Set<JobId>();
  let seq = 0;
  let remembered = prefs.read();
  // Background jobs (the session watcher syncs after every map) must not cover the screen: only a viewer who chose the
  // expanded tray gets it back for a new job; everyone else sees the edge tab's running count.
  const autoOpen = () => remembered === true;

  return createStore<JobTrayState>()((set, get) => ({
    jobs: new Map(),
    open: remembered === true,

    applyProgress: (progress) => {
      if (dismissed.has(progress.jobId)) {
        return;
      }
      const current = get().jobs.get(progress.jobId);
      // Throttled progress can trail the JobFinished event; a finished job never goes back to running.
      if (current !== undefined && isFinished(current.status)) {
        return;
      }
      const jobs = new Map(get().jobs);
      jobs.set(progress.jobId, {
        ...(current ?? blank(progress.jobId)),
        kind: progress.kind,
        status: "running",
        stage: progress.stage,
        done: progress.done,
        total: progress.total,
        etaMs: progress.etaMs,
      });
      set({ jobs, open: get().open || (current === undefined && autoOpen()) });
    },

    applyFinished: (finished) => {
      if (dismissed.has(finished.jobId)) {
        return;
      }
      const jobs = new Map(get().jobs);
      const current = jobs.get(finished.jobId) ?? blank(finished.jobId);
      seq += 1;
      jobs.set(finished.jobId, {
        ...current,
        status: finished.status,
        failedItems: finished.failedItems,
        etaMs: null,
        finishedSeq: current.finishedSeq ?? seq,
      });
      evictOldestFinished(jobs);
      set({ jobs });
    },

    hydrate: (list) => {
      const jobs = new Map(get().jobs);
      let sawNewActive = false;
      const byEnd = [...list].sort((a, b) => (a.ended ?? "").localeCompare(b.ended ?? ""));
      for (const dto of byEnd) {
        if (dismissed.has(dto.id)) {
          continue;
        }
        const known = jobs.get(dto.id);
        const current = known ?? blank(dto.id);
        const finished = isFinished(dto.status);
        sawNewActive ||= known === undefined && !finished;
        let finishedSeq = current.finishedSeq;
        if (finished && finishedSeq === null) {
          seq += 1;
          finishedSeq = seq;
        }
        jobs.set(dto.id, {
          ...current,
          kind: dto.kind,
          // Live events are newer than the list for a job that is still running here.
          status: isFinished(current.status) ? current.status : dto.status,
          failedItems: dto.summary?.counters.failedItems ?? current.failedItems,
          error: dto.error,
          finishedSeq,
        });
      }
      evictOldestFinished(jobs);
      // A job already running at startup (or after a lagged stream) should be visible, like a freshly started one.
      set({ jobs, open: get().open || (sawNewActive && autoOpen()) });
    },

    dismiss: (id) => {
      dismissed.add(id);
      const jobs = new Map(get().jobs);
      jobs.delete(id);
      set({ jobs });
    },

    setOpen: (open) => {
      set({ open });
    },

    chooseOpen: (open) => {
      remembered = open;
      prefs.write(open);
      set({ open });
    },

    reset: () => {
      dismissed = new Set();
      seq = 0;
      remembered = prefs.read();
      set({ jobs: new Map(), open: remembered === true });
    },
  }));
}
