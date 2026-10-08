import { queryOptions } from "@tanstack/react-query";
import { commands, type EntryRefDto, type MergeModeDto } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { qk } from "@/shared/queryKeys";

export const previewKeys = {
  all: qk("preview"),
  skill: (entry: EntryRefDto, keymode: number, merge: MergeModeDto | null) => qk("preview", "skill", entry, keymode, merge),
};

export function skillPreviewQuery(entry: EntryRefDto, keymode: number, merge: MergeModeDto | null) {
  return queryOptions({
    queryKey: previewKeys.skill(entry, keymode, merge),
    queryFn: () => call(commands.previewSkill(entry, keymode, merge)),
  });
}
