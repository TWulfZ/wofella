// Test harness that boots the real route tree, providers and first-run guard over mocked IPC.
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { render, waitFor } from "@testing-library/react";
import { expect } from "vitest";
import type { SetupStatusDto } from "@/ipc/bindings";
import { mockCommands, type CommandHandlers, type PluginHandlers } from "@/ipc/mocks";
import { AppProviders } from "./providers";
import { createQueryClient } from "./queryClient";
import { createAppRouter } from "./router";

export const TEST_INSTALL = {
  id: 1,
  rootPath: "/mnt/e/Games/osu!",
  osuDbVersion: 20260924,
  detectedAt: "2026-09-28T10:00:00.000Z",
};

export function setupStatus(overrides: Partial<SetupStatusDto> = {}): SetupStatusDto {
  return {
    install: TEST_INSTALL,
    identityReady: true,
    dataDir: "/home/pilot/.local/share/wolluf",
    logsDir: "/home/pilot/.local/share/wolluf/logs",
    appVersion: "0.1.0",
    lastSync: null,
    ...overrides,
  };
}

export async function bootApp(path: string, handlers: CommandHandlers = {}, plugins: PluginHandlers = {}) {
  const calls = mockCommands({ setupStatus: () => setupStatus(), jobsList: () => [], ...handlers }, plugins);
  const queryClient = createQueryClient();
  const router = createAppRouter(queryClient, createMemoryHistory({ initialEntries: [path] }));
  const view = render(
    <AppProviders queryClient={queryClient}>
      <RouterProvider router={router} />
    </AppProviders>,
  );
  await waitFor(() => {
    expect(router.state.status).toBe("idle");
  });
  return { calls, queryClient, router, view };
}

export function leafRouteId(router: Awaited<ReturnType<typeof bootApp>>["router"]): string | undefined {
  return router.state.matches.at(-1)?.routeId;
}
