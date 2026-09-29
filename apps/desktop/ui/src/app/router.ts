import type { QueryClient } from "@tanstack/react-query";
import { createRouter, type RouterHistory } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import type { RouterContext } from "@/shared/router";
import { RouteErrorView } from "./ErrorBoundary";

export function createAppRouter(queryClient: QueryClient, history?: RouterHistory) {
  return createRouter({
    routeTree,
    context: { queryClient } satisfies RouterContext,
    ...(history === undefined ? {} : { history }),
    defaultPreload: "intent",
    // Query owns backend data freshness; the router must not cache loader results on top of it.
    defaultPreloadStaleTime: 0,
    defaultErrorComponent: RouteErrorView,
  });
}

declare module "@tanstack/react-router" {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
