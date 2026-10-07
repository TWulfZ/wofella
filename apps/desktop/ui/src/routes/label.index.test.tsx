import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bootApp, leafRouteId } from "@/app/testing";
import { HOLD_BUTTON_PARAMS } from "@/features/label";
import type { ChartWindowDto, LabelProgressDto, LabelStatsDto, LabelWindowDto, SessionPlayDto } from "@/ipc/bindings";
import type { CommandHandlers, MockCall } from "@/ipc/mocks";

const MD5 = "c".repeat(32);

const WINDOW: LabelWindowDto = {
  anchor: { md5: MD5, t0Ms: 1000, t1Ms: 5000, cols: [1, 2, 3, 4, 5, 6, 7] },
  title: "Gamma Song",
  artist: "Artist",
  version: "Insane",
  creator: "Mapper",
  stars: 4.5,
  level: null,
  stratum: "dan_07/nps_2",
  played: true,
};

function chartWindow(md5: string, fromMs: number, toMs: number): ChartWindowDto {
  return {
    md5,
    keymode: 7,
    fromMs,
    toMs,
    notes: [],
    timing: [],
    layout: { id: "k7.313_right_thumb", columns: [] },
    chartSpan: { firstMs: 0, endMs: 90_000 },
    audioFilename: "audio.mp3",
  };
}

const STATS: LabelStatsDto = {
  total: 37,
  noPattern: 0,
  mixed: 0,
  unsure: 0,
  thumbLeft: 0,
  thumbRight: 0,
  perPattern: [],
  perAxis: [],
  perStratum: [],
  blind: 0,
  perSelection: [],
};

const PROGRESS: LabelProgressDto = {
  goldTotal: 37,
  goldNoPattern: 0,
  goldBlind: 0,
  perSelection: [],
  perPattern: [],
  perAxis: [],
  sessionLabels: 0,
  perDay: [],
  recent: [],
};

class Noop {
  observe(): void {
    // jsdom does no layout.
  }
  unobserve(): void {
    // See observe.
  }
  disconnect(): void {
    // See observe.
  }
}

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", Noop);
  vi.stubGlobal("IntersectionObserver", Noop);
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const HANDLERS: CommandHandlers = {
  labelTaxonomy: () => [],
  labelStats: () => STATS,
  labelPatternExamples: () => [],
  settingsGetHandLayout: () => "k7.313_right_thumb",
  labelWindowAt: () => WINDOW,
  labelSample: () => null,
  chartWindow: (args) => chartWindow(String(args["md5"]), Number(args["fromMs"]), Number(args["toMs"])),
  chartAudio: () => {
    throw new Error("no audio in this test");
  },
  chartBackground: () => null,
  chartDetails: () => {
    throw new Error("details not needed");
  },
  labelChartTimeline: () => ({ firstMs: 0, endMs: 90_000, density: [], labelled: [] }),
  skinList: () => ({ skins: [], current: null, maniaSpeed: null, maniaSpeedBpmScale: null }),
  labelProgress: () => PROGRESS,
};

function argsOf(calls: MockCall[], cmd: string): Record<string, unknown>[] {
  return calls.filter((c) => c.cmd === cmd).map((c) => c.args);
}

describe("/label", () => {
  it("keeps the Label screen at /label", async () => {
    const { router } = await bootApp("/label", HANDLERS);
    expect(leafRouteId(router)).toBe("/label/");
  });

  it("opens the chart named by ?chart= and drops the parameter from the address", async () => {
    const { calls, router } = await bootApp(`/label?chart=${MD5}`, HANDLERS);
    expect(await screen.findByRole("heading", { name: "Gamma Song" })).toBeInTheDocument();
    const requests = argsOf(calls, "label_window_at");
    expect(requests).toHaveLength(1);
    expect(requests[0]?.["req"]).toMatchObject({ keymode: 7, md5: MD5, windowMs: null, exclude: [] });
    await waitFor(() => {
      expect(router.state.location.search).not.toHaveProperty("chart");
    });
  });

  it("ignores a ?chart= that is not an md5", async () => {
    const { calls } = await bootApp("/label?chart=nope", HANDLERS);
    await waitFor(() => {
      expect(argsOf(calls, "label_sample")).toHaveLength(1);
    });
    expect(argsOf(calls, "label_window_at")).toHaveLength(0);
  });

  it("shows the labelling progress from the map card's counters, linking to Progress", async () => {
    const { router } = await bootApp(`/label?chart=${MD5}`, HANDLERS);
    await screen.findByRole("heading", { name: "Gamma Song" });
    await userEvent.click(screen.getByRole("button", { name: "Labelling progress" }));
    await userEvent.click(await screen.findByRole("link", { name: "See progress" }));
    await waitFor(() => {
      expect(leafRouteId(router)).toBe("/label/progress");
    });
  });

  it("offers the session map's dominant pattern under the map card; a gold save neither answers it nor goes stale", async () => {
    const play: SessionPlayDto = {
      playId: "cc01",
      md5: MD5,
      playedAt: "2026-10-07T11:00:00.000Z",
      title: "Gamma Song",
      artist: "Artist",
      version: "Insane",
      creator: "Mapper",
      stars: 4.5,
      keymode: 7,
      setId: null,
      label: null,
      goldWindows: 0,
    };
    const { calls } = await bootApp(`/label?chart=${MD5}`, {
      ...HANDLERS,
      sessionPlays: () => ({ startedAt: "2026-10-07T10:00:00.000Z", plays: [play] }),
      labelSubmit: () => ({ id: "01GOLD" }),
      sessionLabelSubmit: () => ({ id: "01SESSION" }),
    });
    await screen.findByRole("heading", { name: "Gamma Song" });
    const stripName = { name: "Map from this session: dominant pattern" };
    expect(await within(await screen.findByRole("region", stripName)).findByTestId("session-status")).toHaveTextContent(
      "Pending",
    );
    const sessionReads = argsOf(calls, "session_plays").length;

    const answer = screen.getByRole("region", { name: "Answer" });
    await userEvent.click(within(answer).getByRole("button", { name: "No pattern" }));
    await holdPress(within(answer).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(argsOf(calls, "label_submit")).toHaveLength(1);
    });
    await waitFor(() => {
      expect(argsOf(calls, "session_plays").length).toBeGreaterThan(sessionReads);
    });

    // The saved window is left for the next one; back on it, the map still waits for its session answer.
    await userEvent.click(await screen.findByRole("button", { name: "Previous" }));
    const strip = await screen.findByRole("region", stripName);
    expect(within(strip).getByTestId("session-status")).toHaveTextContent("Pending");
    expect(argsOf(calls, "session_label_submit")).toEqual([]);

    await userEvent.click(within(strip).getByRole("button", { name: "No clear pattern" }));
    await holdPress(within(strip).getByRole("button", { name: "Save dominant pattern" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toEqual([
        { req: { keymode: 7, md5: MD5, playId: "cc01", pattern: null } },
      ]);
    });
  });

  it("leaves the strip's pattern grid with Enter saving the gold window, not reopening the grid", async () => {
    const { calls } = await bootApp(`/label?chart=${MD5}`, {
      ...HANDLERS,
      labelTaxonomy: () => [
        { id: "7k.regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "two notes in one column" },
      ],
      sessionPlays: () => ({ startedAt: "2026-10-07T10:00:00.000Z", plays: [SESSION_PLAY] }),
      labelSubmit: () => ({ id: "01GOLD" }),
    });
    await screen.findByRole("heading", { name: "Gamma Song" });
    const answer = screen.getByRole("region", { name: "Answer" });
    await userEvent.click(within(answer).getByRole("button", { name: "No pattern" }));
    const strip = await screen.findByRole("region", { name: "Map from this session: dominant pattern" });
    await userEvent.click(
      await within(strip).findByRole("button", { name: "Dominant pattern of Gamma Song: Choose pattern" }),
    );
    const picker = await screen.findByRole("dialog", { name: "Dominant pattern of Gamma Song" });
    await userEvent.type(within(picker).getByRole("searchbox", { name: "Search patterns" }), "mini");
    await userEvent.click(within(picker).getByRole("button", { name: "mj minijack" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    await userEvent.keyboard("{Enter>}");
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, HOLD_BUTTON_PARAMS.defaultHoldMs + 100));
    });
    await userEvent.keyboard("{/Enter}");
    await waitFor(() => {
      expect(argsOf(calls, "label_submit")).toHaveLength(1);
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});

const SESSION_PLAY: SessionPlayDto = {
  playId: "cc01",
  md5: MD5,
  playedAt: "2026-10-07T11:00:00.000Z",
  title: "Gamma Song",
  artist: "Artist",
  version: "Insane",
  creator: "Mapper",
  stars: 4.5,
  keymode: 7,
  setId: null,
  label: null,
  goldWindows: 0,
};

async function holdPress(button: HTMLElement): Promise<void> {
  fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, HOLD_BUTTON_PARAMS.defaultHoldMs + 100));
  });
  fireEvent.pointerUp(button, { button: 0, pointerId: 1 });
}
