import { queryOptions } from "@tanstack/react-query";
import { commands } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { qk } from "@/shared/queryKeys";

export const rateCopyKeys = {
  // Under `library`: a sync that changes the set folder must re-plan an open preview.
  plan: (md5: string, rateMilli: number, nightcore: boolean) => qk("library", "rateCopyPlan", md5, rateMilli, nightcore),
};

export function rateCopyPlanQuery(md5: string, rateMilli: number, nightcore: boolean) {
  return queryOptions({
    queryKey: rateCopyKeys.plan(md5, rateMilli, nightcore),
    queryFn: () => call(commands.rateCopyPlan(md5, rateMilli, nightcore)),
    // Every plan mints a short-lived preview id: a reopened dialog must plan afresh, never reuse a cached one.
    gcTime: 0,
    staleTime: 0,
    refetchOnWindowFocus: false,
  });
}
