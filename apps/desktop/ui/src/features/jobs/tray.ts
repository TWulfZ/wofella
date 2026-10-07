import { useStore } from "zustand";
import type { JobEventSink } from "@/ipc/eventBridge";
import { createJobTrayStore, type JobTrayState } from "./store";
import { localJobTrayPrefs } from "./trayParams";

// One tray per window: jobs outlive the view that started them, so the state is global (§8).
export const jobTrayStore = createJobTrayStore({ prefs: localJobTrayPrefs });

export const jobTraySink: JobEventSink = {
  applyProgress: (progress) => {
    jobTrayStore.getState().applyProgress(progress);
  },
  applyFinished: (finished) => {
    jobTrayStore.getState().applyFinished(finished);
  },
};

export function useJobTray<T>(selector: (state: JobTrayState) => T): T {
  return useStore(jobTrayStore, selector);
}

export function openJobTray(): void {
  jobTrayStore.getState().setOpen(true);
}

/** Clears the global tray; the harness calls it so one booted app never sees another's jobs. */
export function resetJobTray(): void {
  jobTrayStore.getState().reset();
}
