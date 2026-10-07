import { queryOptions, useMutation, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/ipc/bindings";
import { call } from "@/ipc/client";

// Shared with the label screen: whoever writes the preference invalidates `all`, so every keymode's reader redraws.
export const handLayoutKeys = {
  all: ["settings", "handLayout"] as const,
  current: (keymode: number) => ["settings", "handLayout", keymode] as const,
  // "handLayouts" is not under `all`: the preset list never changes with the choice, so saving does not refetch it.
  presets: (keymode: number) => ["settings", "handLayouts", keymode] as const,
};

export function handLayoutQuery(keymode: number) {
  return queryOptions({
    queryKey: handLayoutKeys.current(keymode),
    queryFn: () => call(commands.settingsGetHandLayout(keymode)),
    staleTime: Infinity,
  });
}

export function handLayoutsQuery(keymode: number) {
  return queryOptions({
    queryKey: handLayoutKeys.presets(keymode),
    queryFn: () => call(commands.settingsHandLayouts(keymode)),
  });
}

export function useSetHandLayout(keymode: number) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (layoutId: string) => call(commands.settingsSetHandLayout(keymode, layoutId)),
    onSuccess: async (_data, layoutId) => {
      // Seeded first so the checked option does not flick back to the old value while the refetch is in flight.
      queryClient.setQueryData(handLayoutKeys.current(keymode), layoutId);
      await queryClient.invalidateQueries({ queryKey: handLayoutKeys.all });
    },
  });
}

export const sessionNotifyKey = ["settings", "sessionNotify"] as const;

export function sessionNotifyQuery() {
  return queryOptions({
    queryKey: sessionNotifyKey,
    queryFn: () => call(commands.settingsGetSessionNotify()),
    staleTime: Infinity,
  });
}

export function useSetSessionNotify() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (on: boolean) => call(commands.settingsSetSessionNotify(on)),
    onSuccess: (_data, on) => {
      queryClient.setQueryData(sessionNotifyKey, on);
    },
  });
}
