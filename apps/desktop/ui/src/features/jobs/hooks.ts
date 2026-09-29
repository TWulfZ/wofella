import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import type { JobKindDto } from "@/ipc/bindings";
import { jobsListQuery } from "./queries";
import { isFinished, type TrayJob } from "./store";
import { jobTrayStore, useJobTray } from "./tray";

/** Tray jobs, hydrated from jobs_list; re-hydrates whenever DataChanged{jobs} refetches the list (lag recovery). */
export function useHydratedJobs(): ReadonlyMap<string, TrayJob> {
  const list = useQuery(jobsListQuery());
  useEffect(() => {
    if (list.data !== undefined) {
      jobTrayStore.getState().hydrate(list.data);
    }
  }, [list.data]);
  return useJobTray((s) => s.jobs);
}

export function useRunningJobKinds(): ReadonlySet<JobKindDto> {
  const jobs = useHydratedJobs();
  return useMemo(() => {
    const kinds = new Set<JobKindDto>();
    for (const job of jobs.values()) {
      if (job.kind !== null && !isFinished(job.status)) {
        kinds.add(job.kind);
      }
    }
    return kinds;
  }, [jobs]);
}
