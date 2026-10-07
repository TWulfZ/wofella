import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { LabelProgressDto, SessionPlayDto } from "@/ipc/bindings";
import { type CommandHandlers, mockCommands, mockIpcError } from "@/ipc/mocks";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { ProgressSummary } from "./ProgressSummary";

const PROGRESS: LabelProgressDto = {
  goldTotal: 42,
  goldNoPattern: 0,
  goldBlind: 0,
  perSelection: [],
  perPattern: [],
  perAxis: [],
  sessionLabels: 6,
  perDay: [
    { day: "2026-10-06", gold: 9, session: 9 },
    { day: "2026-10-07", gold: 2, session: 1 },
  ],
  recent: [],
};

function play(n: number, labelled = false): SessionPlayDto {
  const md5 = String(n).repeat(32).slice(0, 32);
  return {
    playId: `0${n}`,
    md5,
    playedAt: "2026-10-07T11:00:00.000Z",
    title: `Song ${n}`,
    artist: "Artist",
    version: "Hard",
    creator: "Mapper",
    stars: null,
    keymode: 7,
    setId: null,
    label: labelled ? { eventId: `01E${n}`, pattern: null, at: "2026-10-07T11:10:00.000Z" } : null,
    goldWindows: 0,
  };
}

function renderSummary(extra: CommandHandlers = {}) {
  mockCommands({
    labelProgress: () => PROGRESS,
    sessionPlays: () => ({ startedAt: "2026-10-07T10:00:00.000Z", plays: [1, 2, 3, 4, 5].map((n) => play(n, n === 5)) }),
    ...extra,
  });
  return renderWithRouter(<ProgressSummary keymode={7} pendingShown={2} />, { path: "/label" });
}

describe("ProgressSummary", () => {
  it("sums up the gold set, today and the session, naming the first pending maps", async () => {
    renderSummary();
    const summary = await screen.findByRole("region", { name: "Labelling progress" });
    await waitFor(() => {
      expect(within(summary).getByText("42")).toBeInTheDocument();
    });
    expect(within(summary).getByText("3")).toBeInTheDocument();
    expect(within(summary).getByText("1 labelled, 4 pending")).toBeInTheDocument();
    const pending = within(summary).getByRole("list", { name: "Waiting for a label" });
    expect(within(pending).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["Song 1", "Song 2"]);
    expect(within(summary).getByText("and 2 more")).toBeInTheDocument();
  });

  it("links to the progress page", async () => {
    const { router } = renderSummary();
    await userEvent.click(await screen.findByRole("link", { name: "See progress" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/label/progress");
    });
  });

  it("words an error instead of numbers", async () => {
    renderSummary({ labelProgress: () => mockIpcError("INTERNAL"), sessionPlays: () => mockIpcError("INTERNAL") });
    const summary = await screen.findByRole("region", { name: "Labelling progress" });
    expect(await within(summary).findByRole("alert")).toBeInTheDocument();
    expect(within(summary).getByRole("link", { name: "See progress" })).toBeInTheDocument();
  });
});
