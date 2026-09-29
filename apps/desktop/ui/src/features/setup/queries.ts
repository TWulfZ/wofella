import { queryOptions } from "@tanstack/react-query";
import { commands } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { qk } from "@/shared/queryKeys";

export const setupKeys = {
  all: qk("setup"),
  status: () => qk("setup", "status"),
  candidates: () => qk("setup", "candidates"),
};

export function setupStatusQuery() {
  return queryOptions({ queryKey: setupKeys.status(), queryFn: () => call(commands.setupStatus()) });
}

export function installCandidatesQuery() {
  return queryOptions({ queryKey: setupKeys.candidates(), queryFn: () => call(commands.setupDetectInstalls()) });
}
