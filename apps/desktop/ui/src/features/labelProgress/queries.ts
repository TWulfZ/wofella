import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type SessionLabelSubmitDto } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { LABEL_PROGRESS_ROOT, SESSION_PLAYS_ROOT } from "@/shared/queryKeys";
import { sessionMapCounts } from "./model";

export const labelProgressKeys = {
  sessionPlays: (keymode: number) => [...SESSION_PLAYS_ROOT, keymode] as const,
  progress: (keymode: number, utcOffsetMin: number) => [...LABEL_PROGRESS_ROOT, keymode, utcOffsetMin] as const,
};

/** Minutes east of UTC, as `label_progress` cuts its days. */
export function localUtcOffsetMin(at: Date = new Date()): number {
  // getTimezoneOffset counts west of UTC; `|| 0` turns UTC's -0 into 0 so the query key stays stable.
  return -at.getTimezoneOffset() || 0;
}

export function sessionPlaysQuery(keymode: number) {
  return queryOptions({
    queryKey: labelProgressKeys.sessionPlays(keymode),
    queryFn: () => call(commands.sessionPlays(keymode)),
  });
}

export function labelProgressQuery(keymode: number, utcOffsetMin: number) {
  return queryOptions({
    queryKey: labelProgressKeys.progress(keymode, utcOffsetMin),
    queryFn: () => call(commands.labelProgress(keymode, utcOffsetMin)),
  });
}

/** Maps played since wolluf opened that still have no answer; undefined until the list answers. */
export function usePendingSessionMaps(keymode: number, enabled = true): number | undefined {
  const plays = useQuery({ ...sessionPlaysQuery(keymode), enabled });
  return plays.data === undefined ? undefined : sessionMapCounts(plays.data.plays).pending;
}

export function useSessionLabelMutations() {
  const queryClient = useQueryClient();
  const refresh = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: SESSION_PLAYS_ROOT }),
      queryClient.invalidateQueries({ queryKey: LABEL_PROGRESS_ROOT }),
    ]);
  return {
    submit: useMutation({
      mutationFn: (req: SessionLabelSubmitDto) => call(commands.sessionLabelSubmit(req)),
      onSuccess: refresh,
    }),
    undo: useMutation({
      mutationFn: (eventId: string) => call(commands.sessionLabelUndo(eventId)),
      onSuccess: refresh,
    }),
  };
}
