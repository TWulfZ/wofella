// Test harness that boots the real route tree, providers and first-run guard over mocked IPC.
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { render, waitFor } from "@testing-library/react";
import { expect } from "vitest";
import type { ChartMsdDto, KeymodeDto, SetupStatusDto } from "@/ipc/bindings";
import { resetJobTray } from "@/features/jobs";
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

export const TEST_KEYMODES: KeymodeDto[] = [
  { keymode: 4, hasPatterns: false, calculators: ["minacalc"], defaultLayout: "k4.generic", hasThumb: false },
  { keymode: 7, hasPatterns: true, calculators: ["minacalc"], defaultLayout: "k7.313_right_thumb", hasThumb: true },
];

export const TEST_CHART_MSD: ChartMsdDto = {
  md5: "0".repeat(32),
  status: "rated",
  holdSharePermille: 42,
  calcVersion: 527,
  skillsets: ["overall", "stream", "jumpstream", "handstream", "stamina", "jackspeed", "chordjack", "technical"],
  rates: [{ rateMilli: 1000, centi: [2345, 2210, 2105, 1990, 1888, 1601, 1507, 1802] }],
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
  resetJobTray();
  const calls = mockCommands(
    {
      setupStatus: () => setupStatus(),
      setupDetectInstalls: () => [],
      jobsList: () => [],
      playersListAliases: () => ({ selectionVersion: 1, cfgUsernameAvailable: true, wizardNeeded: false, aliases: [] }),
      // The shell's nav badge asks on every route.
      sessionPlays: () => ({ startedAt: "2026-10-07T10:00:00.000Z", plays: [] }),
      // Settings always shows the opt-in switch; it is off until the user turns it on.
      settingsGetSessionNotify: () => false,
      // And the recommendations card, off until the user widens the rates.
      settingsGetRecsAnyRate: () => false,
      // The header's keymode switcher asks on every route; two keymodes so it shows.
      metaKeymodes: () => TEST_KEYMODES,
      chartMsd: (args) => ({ ...TEST_CHART_MSD, md5: String(args["md5"]) }),
      ...handlers,
    },
    plugins,
  );
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
