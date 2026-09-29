import type { QueryClient } from "@tanstack/react-query";
import { events, type JobFinishedDto, type JobProgressDto } from "./bindings";

// Implemented by the jobs feature's tray store; declared here because ipc/ may not import features (D14).
export interface JobEventSink {
  applyProgress: (progress: JobProgressDto) => void;
  applyFinished: (finished: JobFinishedDto) => void;
}

export async function startEventBridge(queryClient: QueryClient, sink: JobEventSink): Promise<() => void> {
  const unlisteners = await Promise.all([
    events.dataChanged.listen(({ payload }) => {
      for (const domain of payload.domains) {
        void queryClient.invalidateQueries({ queryKey: [domain] });
      }
    }),
    events.jobProgress.listen(({ payload }) => {
      sink.applyProgress(payload);
    }),
    events.jobFinished.listen(({ payload }) => {
      sink.applyFinished(payload);
    }),
  ]);
  return () => {
    for (const unlisten of unlisteners) {
      unlisten();
    }
  };
}
