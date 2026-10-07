import { beforeEach, describe, expect, it, vi } from "vitest";
import type { JobDto, JobProgressDto } from "@/ipc/bindings";
import { createJobTrayStore, FINISHED_CAP, type JobTrayStore } from "./store";

function progress(jobId: string, done = 1, total = 10): JobProgressDto {
  return { jobId, kind: "sync_plays", stage: "ingest", done, total, etaMs: 2000 };
}

function jobDto(id: string, status: JobDto["status"], ended: string | null = null): JobDto {
  return {
    id,
    kind: "sync_plays",
    status,
    started: "2026-09-28T10:00:00.000Z",
    ended,
    summary:
      status === "ok"
        ? {
            kind: "sync_plays",
            counters: {
              playsNew: 1,
              playsReplayOnly: 0,
              playsExisting: 0,
              conflicts: 0,
              skippedNonMania: 0,
              replaysLinked: 0,
              osgLinked: 0,
              chartsArchived: 0,
              chartUnavailable: 0,
              chartMd5Mismatch: 0,
              orphanReplays: 0,
              failedItems: 4,
            },
          }
        : null,
    error: status === "failed" ? { code: "OSU_RUNNING", messageKey: "error.code.OSU_RUNNING" } : null,
  };
}

let store: JobTrayStore;
const state = () => store.getState();

beforeEach(() => {
  store = createJobTrayStore();
});

describe("job tray store", () => {
  it("creates a running job from its first progress event, leaving the tray collapsed behind its tab", () => {
    state().applyProgress(progress("a", 3, 12));
    expect(state().jobs.get("a")).toMatchObject({ status: "running", kind: "sync_plays", done: 3, total: 12, etaMs: 2000 });
    expect(state().open).toBe(false);
  });

  it("a background job does not reopen the tray after it auto-collapsed, by event or by hydration", () => {
    state().setOpen(true);
    state().applyProgress(progress("a"));
    state().applyFinished({ jobId: "a", status: "ok", failedItems: 0 });
    state().setOpen(false);
    state().applyProgress(progress("b"));
    state().hydrate([jobDto("c", "running")]);
    expect(state().open).toBe(false);
  });

  it("reopens for each new job only for a viewer who chose to keep it expanded", () => {
    store = createJobTrayStore({ prefs: { read: () => true, write: () => undefined } });
    state().setOpen(false);
    state().applyProgress(progress("a"));
    expect(state().open).toBe(true);
    state().setOpen(false);
    state().hydrate([jobDto("b", "running")]);
    expect(state().open).toBe(true);
  });

  it("marks a job finished with its failed-item count", () => {
    state().applyProgress(progress("a"));
    state().applyFinished({ jobId: "a", status: "ok", failedItems: 37 });
    expect(state().jobs.get("a")).toMatchObject({ status: "ok", failedItems: 37, kind: "sync_plays" });
  });

  it("keeps a finished event for a job it never saw, with an unknown kind", () => {
    state().applyFinished({ jobId: "z", status: "cancelled", failedItems: 0 });
    expect(state().jobs.get("z")).toMatchObject({ status: "cancelled", kind: null });
  });

  it("ignores progress that arrives after the job finished", () => {
    state().applyFinished({ jobId: "a", status: "ok", failedItems: 0 });
    state().applyProgress(progress("a", 9, 10));
    expect(state().jobs.get("a")?.status).toBe("ok");
  });

  it("hydrates running and finished jobs from jobs_list, keeping live progress", () => {
    state().applyProgress(progress("run", 5, 50));
    state().hydrate([jobDto("run", "running"), jobDto("done", "ok", "2026-09-28T10:05:00.000Z"), jobDto("bad", "failed", "2026-09-28T10:06:00.000Z")]);
    expect(state().jobs.get("run")).toMatchObject({ status: "running", done: 5, total: 50 });
    expect(state().jobs.get("done")).toMatchObject({ status: "ok", failedItems: 4 });
    expect(state().jobs.get("bad")).toMatchObject({ status: "failed", error: { code: "OSU_RUNNING" } });
  });

  it("hydration does not resurrect dismissed jobs", () => {
    state().applyFinished({ jobId: "a", status: "ok", failedItems: 0 });
    state().dismiss("a");
    state().hydrate([jobDto("a", "ok", "2026-09-28T10:05:00.000Z")]);
    expect(state().jobs.has("a")).toBe(false);
  });

  it(`keeps at most ${FINISHED_CAP} finished jobs, evicting the oldest`, () => {
    state().applyProgress(progress("live"));
    for (let i = 0; i <= FINISHED_CAP; i += 1) {
      state().applyFinished({ jobId: `f${i}`, status: "ok", failedItems: 0 });
    }
    const finished = [...state().jobs.values()].filter((j) => j.status === "ok");
    expect(finished).toHaveLength(FINISHED_CAP);
    expect(state().jobs.has("f0")).toBe(false);
    expect(state().jobs.has(`f${FINISHED_CAP}`)).toBe(true);
    expect(state().jobs.has("live")).toBe(true);
  });

  it("orders hydrated finished jobs by end time for eviction", () => {
    const history = Array.from({ length: FINISHED_CAP + 1 }, (_, i) =>
      jobDto(`h${i}`, "ok", `2026-09-28T10:${String(i).padStart(2, "0")}:00.000Z`),
    ).reverse();
    state().hydrate(history);
    expect(state().jobs.has("h0")).toBe(false);
    expect(state().jobs.size).toBe(FINISHED_CAP);
  });

  it("a remembered collapse keeps new jobs from expanding the tray", () => {
    store = createJobTrayStore({ prefs: { read: () => false, write: () => undefined } });
    state().applyProgress(progress("a"));
    state().hydrate([jobDto("b", "running")]);
    expect(state().open).toBe(false);
  });

  it("starts from the remembered choice and persists only the user's choices", () => {
    const write = vi.fn();
    store = createJobTrayStore({ prefs: { read: () => true, write } });
    expect(state().open).toBe(true);

    state().setOpen(false);
    expect(write).not.toHaveBeenCalled();

    state().chooseOpen(true);
    expect(state().open).toBe(true);
    expect(write).toHaveBeenCalledWith(true);
  });

  it("reset re-reads the remembered choice", () => {
    let remembered: boolean | null = null;
    store = createJobTrayStore({ prefs: { read: () => remembered, write: () => undefined } });
    expect(state().open).toBe(false);
    remembered = true;
    state().reset();
    expect(state().open).toBe(true);
  });
});
