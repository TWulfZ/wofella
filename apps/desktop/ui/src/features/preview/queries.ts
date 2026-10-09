import { queryOptions, useMutation, useQueryClient } from "@tanstack/react-query";
import { commands, type EntryRefDto, type MergeModeDto, type RecsModeDto } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { qk } from "@/shared/queryKeys";

export const previewKeys = {
  all: qk("preview"),
  skill: (entry: EntryRefDto, keymode: number, merge: MergeModeDto | null) => qk("preview", "skill", entry, keymode, merge),
  recsAll: qk("preview", "recs"),
  recs: (entry: EntryRefDto, keymode: number, mode: RecsModeDto, skillset: string | null, merge: MergeModeDto | null) =>
    qk("preview", "recs", entry, keymode, mode, skillset, merge),
};

export function skillPreviewQuery(entry: EntryRefDto, keymode: number, merge: MergeModeDto | null) {
  return queryOptions({
    queryKey: previewKeys.skill(entry, keymode, merge),
    queryFn: () => call(commands.previewSkill(entry, keymode, merge)),
  });
}

export function recsPreviewQuery(
  entry: EntryRefDto,
  keymode: number,
  mode: RecsModeDto,
  skillset: string | null,
  merge: MergeModeDto | null,
) {
  return queryOptions({
    queryKey: previewKeys.recs(entry, keymode, mode, skillset, merge),
    queryFn: () => call(commands.previewRecs(entry, keymode, mode, skillset, merge)),
  });
}

export const recsAnyRateKey = ["settings", "recsAnyRate"] as const;

export function recsAnyRateQuery() {
  return queryOptions({
    queryKey: recsAnyRateKey,
    queryFn: () => call(commands.settingsGetRecsAnyRate()),
    staleTime: Infinity,
  });
}

export function useSetRecsAnyRate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (on: boolean) => call(commands.settingsSetRecsAnyRate(on)),
    onSuccess: async (_data, on) => {
      queryClient.setQueryData(recsAnyRateKey, on);
      // The service reads the setting on every request, so cached lists still hold the other rate set.
      await queryClient.invalidateQueries({ queryKey: previewKeys.recsAll });
    },
  });
}
