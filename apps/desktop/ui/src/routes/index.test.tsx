import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { bootApp, setupStatus } from "@/app/testing";
import type { JobDto } from "@/ipc/bindings";
import { i18n } from "@/shared/i18n";

const SYNC_OK: JobDto = {
  id: "01J0000000000000000000000",
  kind: "sync_plays",
  status: "ok",
  started: "2026-10-05T20:00:00.000Z",
  ended: "2026-10-05T20:01:10.000Z",
  summary: {
    kind: "sync_plays",
    counters: {
      playsNew: 128,
      playsReplayOnly: 4,
      playsExisting: 3019,
      conflicts: 0,
      skippedNonMania: 12,
      replaysLinked: 3011,
      osgLinked: 40,
      chartsArchived: 900,
      chartUnavailable: 0,
      chartMd5Mismatch: 0,
      orphanReplays: 2,
      failedItems: 0,
    },
  },
  error: null,
};

function statTile(label: string): HTMLElement {
  const tile = screen.getByText(label).parentElement;
  if (tile === null) {
    throw new Error(`no tile for ${label}`);
  }
  return tile;
}

describe("/ home", () => {
  it("shows the page subtitle and the osu!.db version of the install", async () => {
    await bootApp("/");
    expect(await screen.findByRole("heading", { name: "Home" })).toBeInTheDocument();
    expect(screen.getByText("Your osu!mania training hub")).toBeInTheDocument();
    expect(screen.getByText("osu!.db 20260924")).toBeInTheDocument();
  });

  it("renders the last sync status and its counters as localized stat tiles", async () => {
    await bootApp("/", { setupStatus: () => setupStatus({ lastSync: SYNC_OK }) });
    expect(await screen.findByText("Finished")).toBeInTheDocument();
    expect(within(statTile("New plays")).getByText("128")).toBeInTheDocument();
    expect(within(statTile("Already known")).getByText("3,019")).toBeInTheDocument();
    expect(within(statTile("Replays linked")).getByText("3,011")).toBeInTheDocument();
  });

  it("formats the counters for the active language", async () => {
    await i18n.changeLanguage("es");
    await bootApp("/", { setupStatus: () => setupStatus({ lastSync: SYNC_OK }) });
    expect(await screen.findByText("Terminada")).toBeInTheDocument();
    expect(within(statTile("Ya conocidas")).getByText("3019")).toBeInTheDocument();
    expect(within(statTile("Partidas nuevas")).getByText("128")).toBeInTheDocument();
    expect(within(statTile("Replays vinculados")).getByText("3011")).toBeInTheDocument();
  });

  it("shows the failed status with its text, not color alone", async () => {
    await bootApp("/", { setupStatus: () => setupStatus({ lastSync: { ...SYNC_OK, status: "failed", summary: null } }) });
    expect(await screen.findByText("Failed")).toBeInTheDocument();
    expect(screen.queryByText("New plays")).not.toBeInTheDocument();
  });
});
