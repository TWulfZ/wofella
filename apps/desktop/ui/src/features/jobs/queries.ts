import { queryOptions } from "@tanstack/react-query";
import { commands } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { qk } from "@/shared/queryKeys";

export const jobKeys = {
  all: qk("jobs"),
  list: () => qk("jobs", "list"),
};

export function jobsListQuery() {
  return queryOptions({ queryKey: jobKeys.list(), queryFn: () => call(commands.jobsList()) });
}
