import { queryOptions } from "@tanstack/react-query";
import { commands } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { qk } from "@/shared/queryKeys";

export const playersKeys = {
  all: qk("players"),
  aliases: () => qk("players", "aliases"),
  profiles: (keymode: number) => qk("players", "profiles", keymode),
};

export function aliasesQuery() {
  return queryOptions({ queryKey: playersKeys.aliases(), queryFn: () => call(commands.playersListAliases()) });
}

export function profilesQuery(keymode: number) {
  return queryOptions({
    queryKey: playersKeys.profiles(keymode),
    queryFn: () => call(commands.playersListProfiles(keymode)),
  });
}

export const metaKeys = {
  keymodes: () => qk("meta", "keymodes"),
};

export function keymodesQuery() {
  return queryOptions({
    queryKey: metaKeys.keymodes(),
    queryFn: () => call(commands.metaKeymodes()),
    // The engine registry is compiled in, so the list cannot change while the app runs.
    staleTime: Infinity,
  });
}
