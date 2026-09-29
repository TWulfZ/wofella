// Test-only: mounts a component inside a throwaway router so slices can assert navigation without the app shell
// (features may not import src/app, D14).
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
  useLocation,
} from "@tanstack/react-router";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";

function LocationProbe() {
  const location = useLocation();
  return <div data-testid="location">{location.pathname}</div>;
}

export function renderWithRouter(ui: ReactNode, { path = "/" }: { path?: string } = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  const rootRoute = createRootRoute({ component: Outlet });
  const routeTree = rootRoute.addChildren([
    createRoute({ getParentRoute: () => rootRoute, path: path.split("?")[0] ?? "/", component: () => ui }),
    createRoute({ getParentRoute: () => rootRoute, path: "$", component: LocationProbe }),
  ]);
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [path] }) });
  const view = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { queryClient, router, view };
}

/** Resolves once the component under test navigated away, returning the new pathname. */
export async function navigatedPath(): Promise<string | null> {
  return (await screen.findByTestId("location")).textContent;
}
