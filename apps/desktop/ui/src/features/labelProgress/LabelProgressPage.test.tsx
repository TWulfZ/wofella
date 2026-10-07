import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  LabelProgressDto,
  PatternDefDto,
  SessionLabelSubmitDto,
  SessionPlayDto,
  SessionPlaysDto,
} from "@/ipc/bindings";
import { type CommandHandlers, type MockCall, mockCommands, mockIpcError } from "@/ipc/mocks";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { i18n } from "@/shared/i18n";
import { difficultyColour, labelKeys } from "@/features/label";
import { LABEL_PROGRESS_PARAMS, LabelProgressPage } from "./LabelProgressPage";

const NOW = Date.parse("2026-10-07T12:00:00.000Z");

const TAXONOMY: PatternDefDto[] = [
  { id: "7k.regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "two notes in one column" },
  { id: "7k.regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "three or more" },
  { id: "7k.regular.stream.jumpstream", axis: "7k.regular.stream", key: "js", description: "stream with jumps" },
  { id: "7k.ln.release.shield", axis: "7k.ln.release", key: "sh", description: "release then press" },
];

const A = "a".repeat(32);
const B = "b".repeat(32);

function day(offset: number, gold: number, session: number) {
  const date = new Date(Date.parse("2026-10-07T00:00:00.000Z") - offset * 86_400_000);
  return { day: date.toISOString().slice(0, 10), gold, session };
}

const PROGRESS: LabelProgressDto = {
  goldTotal: 42,
  goldNoPattern: 3,
  goldBlind: 0,
  perSelection: [],
  perPattern: [
    { key: "7k.ln.release.shield", count: 4 },
    { key: "7k.regular.jack.minijack", count: 12 },
  ],
  perAxis: [
    { key: "7k.ln.release", count: 4 },
    { key: "7k.regular.jack", count: 12 },
  ],
  sessionLabels: 6,
  perDay: [...Array.from({ length: 28 }, (_, i) => day(29 - i, 0, 0)), day(1, 3, 1), day(0, 2, 2)],
  recent: [
    {
      eventId: "01GOLD2",
      md5: A,
      title: "Alpha Song",
      version: "Insane",
      patterns: ["7k.regular.jack.minijack", "7k.ln.release.shield"],
      noPattern: false,
      at: "2026-10-07T11:55:00.000Z",
    },
    {
      eventId: "01GOLD1",
      md5: "f".repeat(32),
      title: null,
      version: null,
      patterns: [],
      noPattern: true,
      at: "2026-10-06T12:00:00.000Z",
    },
  ],
};

function sessionPlay(md5: string, title: string, overrides: Partial<SessionPlayDto> = {}): SessionPlayDto {
  return {
    playId: `${md5.slice(0, 8)}01`,
    md5,
    playedAt: "2026-10-07T11:50:00.000Z",
    title,
    artist: "Artist",
    version: "Hard",
    creator: "Mapper",
    stars: 4.21,
    keymode: 7,
    setId: null,
    label: null,
    goldWindows: 0,
    ...overrides,
  };
}

const PLAY_A = sessionPlay(A, "Alpha Song");
const PLAY_B = sessionPlay(B, "Beta Song", {
  playId: "bbbbbbbb02",
  playedAt: "2026-10-07T11:00:00.000Z",
  label: { eventId: "01SESSIONB", pattern: "7k.regular.jack.minijack", at: "2026-10-07T11:10:00.000Z" },
});

const SESSION: SessionPlaysDto = { startedAt: "2026-10-07T10:00:00.000Z", plays: [PLAY_A, PLAY_B] };

class NoopResizeObserver {
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

// Rows ask for their thumbnail only once revealed; nothing is revealed unless a test says so.
class MockIntersectionObserver {
  static instances: MockIntersectionObserver[] = [];
  readonly targets = new Set<Element>();
  readonly callback: IntersectionObserverCallback;
  readonly options: IntersectionObserverInit | undefined;
  constructor(callback: IntersectionObserverCallback, options?: IntersectionObserverInit) {
    this.callback = callback;
    this.options = options;
    MockIntersectionObserver.instances.push(this);
  }
  observe(target: Element): void {
    this.targets.add(target);
  }
  unobserve(target: Element): void {
    this.targets.delete(target);
  }
  disconnect(): void {
    this.targets.clear();
  }
  takeRecords(): IntersectionObserverEntry[] {
    return [];
  }
}

function reveal(target: Element): void {
  act(() => {
    for (const observer of MockIntersectionObserver.instances.filter((o) => o.targets.has(target))) {
      observer.callback(
        [{ target, isIntersecting: true } as IntersectionObserverEntry],
        observer as unknown as IntersectionObserver,
      );
    }
  });
}

function observed(target: Element): boolean {
  return MockIntersectionObserver.instances.some((o) => o.targets.has(target));
}

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", NoopResizeObserver);
  MockIntersectionObserver.instances = [];
  vi.stubGlobal("IntersectionObserver", MockIntersectionObserver);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function renderPage(extra: CommandHandlers = {}) {
  const calls = mockCommands({
    labelTaxonomy: () => TAXONOMY,
    labelProgress: () => PROGRESS,
    sessionPlays: () => SESSION,
    sessionLabelSubmit: () => ({ id: "01NEW" }),
    sessionLabelUndo: () => null,
    chartBackground: () => null,
    settingsGetHandLayout: () => "k7.313_right_thumb",
    labelPatternExamples: () => [],
    ...extra,
  });
  const rendered = renderWithRouter(
    <LabelProgressPage keymode={7} now={() => NOW} params={LABEL_PROGRESS_PARAMS} />,
    { path: "/label/progress" },
  );
  return { calls, ...rendered };
}

const CLOCK_TICK_MS = 20;

function renderWithClock(now: () => number) {
  const calls = mockCommands({
    labelTaxonomy: () => TAXONOMY,
    labelProgress: () => PROGRESS,
    sessionPlays: () => SESSION,
  });
  renderWithRouter(
    <LabelProgressPage keymode={7} now={now} params={{ ...LABEL_PROGRESS_PARAMS, clockTickMs: CLOCK_TICK_MS }} />,
    { path: "/label/progress" },
  );
  return { calls };
}

function argsOf(calls: MockCall[], cmd: string): Record<string, unknown>[] {
  return calls.filter((c) => c.cmd === cmd).map((c) => c.args);
}

async function wait(ms: number): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

async function sessionRegion(): Promise<HTMLElement> {
  return screen.findByRole("region", { name: "This session" });
}

async function row(title: string): Promise<HTMLElement> {
  const region = await sessionRegion();
  return within(region).findByRole("listitem", { name: new RegExp(title) });
}

describe("LabelProgressPage", () => {
  it("asks for the progress at the local UTC offset and the session of the keymode", async () => {
    const { calls } = renderPage();
    await screen.findByRole("region", { name: "Totals" });
    expect(argsOf(calls, "label_progress")).toEqual([{ keymode: 7, utcOffsetMin: -new Date().getTimezoneOffset() || 0 }]);
    expect(argsOf(calls, "session_plays")).toEqual([{ keymode: 7 }]);
  });

  it("headlines the gold total, the session maps labelled and today's labels", async () => {
    renderPage();
    const totals = await screen.findByRole("region", { name: "Totals" });
    const card = (name: string) => within(totals).getByRole("group", { name });
    await waitFor(() => {
      expect(card("Gold labels")).toHaveTextContent("42");
    });
    expect(card("Session maps labelled")).toHaveTextContent("6");
    expect(card("Labelled today")).toHaveTextContent("4");
    expect(card("Labelled today")).toHaveTextContent("2 gold, 2 session");
  });

  it("splits the gold labels into blind, chosen and unrecorded origins, explained in text", async () => {
    renderPage({
      labelProgress: () => ({
        ...PROGRESS,
        goldBlind: 30,
        perSelection: [
          { selection: null, count: 8 },
          { selection: { pick: "random", window: "sampled" }, count: 10 },
          { selection: { pick: "sampled", window: "sampled" }, count: 20 },
          { selection: { pick: "session", window: "moved" }, count: 1 },
          { selection: { pick: "now_playing", window: "sampled" }, count: 3 },
        ],
      }),
    });
    const totals = await screen.findByRole("region", { name: "Totals" });
    const origins = await within(totals).findByRole("group", { name: "Gold labels by how the window was chosen" });
    await waitFor(() => {
      expect(within(origins).getAllByRole("listitem").map((li) => li.textContent)).toEqual([
        "30 blind",
        "4 chosen",
        "8 with no recorded origin",
      ]);
    });
    expect(within(origins).getByText(/they are what the evaluation uses/)).toBeInTheDocument();
  });

  it("bars the gold labels per axis and pattern under RICE and LN, each with its count as text", async () => {
    renderPage();
    const axes = await screen.findByRole("region", { name: "Gold labels by pattern" });
    const rice = await within(axes).findByRole("region", { name: "RICE" });
    const ln = within(axes).getByRole("region", { name: "LN" });
    expect(within(rice).getByRole("listitem", { name: "Jack: 12" })).toBeInTheDocument();
    expect(within(rice).getByRole("listitem", { name: "minijack: 12" })).toBeInTheDocument();
    expect(within(rice).getByRole("listitem", { name: "longjack: 0" })).toBeInTheDocument();
    expect(within(rice).getByRole("listitem", { name: "Stream: 0" })).toBeInTheDocument();
    expect(within(ln).getByRole("listitem", { name: "LN release: 4" })).toBeInTheDocument();
    expect(within(axes).getByText("No clear pattern: 3")).toBeInTheDocument();
    expect(rice.querySelector("[data-axis-icon='7k.regular.jack']")).not.toBeNull();
  });

  it("charts 30 days of gold and session labels with a text summary", async () => {
    renderPage();
    const activity = await screen.findByRole("region", { name: "Last 30 days" });
    await waitFor(() => {
      expect(within(activity).getByText("Last 30 days: 5 gold labels and 3 session maps, on 2 days.")).toBeInTheDocument();
    });
    expect(activity.querySelectorAll("[data-day]")).toHaveLength(30);
  });

  it("lists recent gold labels with their map, patterns and relative time", async () => {
    renderPage();
    const recent = await screen.findByRole("region", { name: "Recent gold labels" });
    const items = await within(recent).findAllByRole("listitem");
    expect(items[0]).toHaveTextContent("Alpha Song");
    expect(items[0]).toHaveTextContent("Insane");
    expect(items[0]).toHaveTextContent("minijack");
    expect(items[0]).toHaveTextContent("shield");
    expect(items[0]).toHaveTextContent("5 minutes ago");
    expect(items[1]).toHaveTextContent("Map no longer in the library");
    expect(items[1]).toHaveTextContent("No clear pattern");
    expect(items[1]).toHaveTextContent("yesterday");
  });

  it("says when there are no gold labels yet", async () => {
    renderPage({ labelProgress: () => ({ ...PROGRESS, goldTotal: 0, recent: [], perAxis: [], perPattern: [] }) });
    const recent = await screen.findByRole("region", { name: "Recent gold labels" });
    expect(await within(recent).findByText(/No gold labels yet/)).toBeInTheDocument();
  });

  it("shows the progress error with a retry", async () => {
    let fail = true;
    renderPage({
      labelProgress: () => (fail ? mockIpcError("INTERNAL") : PROGRESS),
    });
    const totals = await screen.findByRole("region", { name: "Totals" });
    expect(await within(totals).findByRole("alert")).toBeInTheDocument();
    expect(screen.queryByText("Loading…")).not.toBeInTheDocument();
    expect(screen.getAllByText("Not available right now.").length).toBeGreaterThan(0);
    fail = false;
    await userEvent.click(within(totals).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(within(totals).getByRole("group", { name: "Gold labels" })).toHaveTextContent("42");
    });
  });

  it("lists one row per map, the maps to label first, with how often each was played", async () => {
    const olderA = sessionPlay(A, "Alpha Song", { playId: "aaaaaaaa00", playedAt: "2026-10-07T10:30:00.000Z" });
    const newerB = { ...PLAY_B, playedAt: "2026-10-07T11:55:00.000Z" };
    renderPage({ sessionPlays: () => ({ ...SESSION, plays: [newerB, PLAY_A, olderA] }) });
    const region = await sessionRegion();
    const rows = await within(region).findAllByRole("listitem");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveAccessibleName(expect.stringContaining("Alpha Song"));
    expect(rows[0]).toHaveTextContent("Hard");
    expect(rows[0]).toHaveTextContent("4.21");
    expect(rows[0]).toHaveTextContent("10 minutes ago");
    expect(rows[0]).toHaveTextContent("Pending");
    expect(rows[0]).toHaveTextContent("Played 2 times");
    expect(rows[1]).toHaveTextContent("minijack");
    expect(rows[1]).not.toHaveTextContent(/Played \d+ times/);
    expect(within(within(region).getByRole("list", { name: "To label" })).getAllByRole("listitem")).toEqual([rows[0]]);
    expect(within(within(region).getByRole("list", { name: "Labelled" })).getAllByRole("listitem")).toEqual([rows[1]]);
    expect(within(region).getByText("1 labelled · 1 pending")).toBeInTheDocument();
  });

  it("says the patterns failed to load where they are needed, with a retry, instead of loading forever", async () => {
    let fail = true;
    renderPage({ labelTaxonomy: () => (fail ? mockIpcError("INTERNAL") : TAXONOMY) });
    const axes = await screen.findByRole("region", { name: "Gold labels by pattern" });
    expect(await within(axes).findByRole("alert")).toBeInTheDocument();
    const region = await sessionRegion();
    const alert = await within(region).findByRole("alert");
    expect(alert).toHaveTextContent("The pattern list could not be loaded");
    expect(screen.queryByText("Loading…")).not.toBeInTheDocument();
    fail = false;
    await userEvent.click(within(alert).getByRole("button", { name: "Retry" }));
    expect(await within(axes).findByRole("region", { name: "RICE" })).toBeInTheDocument();
    expect(within(region).queryByRole("alert")).not.toBeInTheDocument();
  });

  it("keeps relative times and today current while the page stays open", async () => {
    let nowMs = NOW;
    const { calls } = renderWithClock(() => nowMs);
    const alpha = await row("Alpha Song");
    expect(alpha).toHaveTextContent("10 minutes ago");
    nowMs = NOW + 50 * 60_000;
    await wait(CLOCK_TICK_MS * 3);
    expect(alpha).toHaveTextContent("1 hour ago");
    expect(argsOf(calls, "label_progress")).toHaveLength(1);
    nowMs = NOW + 86_400_000;
    await waitFor(() => {
      expect(argsOf(calls, "label_progress")).toHaveLength(2);
    });
  });

  it("words counts of one in the singular", async () => {
    renderPage({
      labelProgress: () => ({ ...PROGRESS, perDay: [...Array.from({ length: 29 }, (_, i) => day(29 - i, 0, 0)), day(0, 1, 1)] }),
      sessionPlays: () => ({ ...SESSION, plays: [PLAY_A] }),
    });
    const activity = await screen.findByRole("region", { name: "Last 30 days" });
    expect(await within(activity).findByText("Last 30 days: 1 gold label and 1 session map, on 1 day.")).toBeInTheDocument();
  });

  it("shows the empty session state when nothing was played since opening", async () => {
    renderPage({ sessionPlays: () => ({ ...SESSION, plays: [] }) });
    const region = await sessionRegion();
    expect(await within(region).findByText(/Play a map in osu! with wolluf open/)).toBeInTheDocument();
  });

  it("shows the session error", async () => {
    renderPage({ sessionPlays: () => mockIpcError("INTERNAL") });
    const region = await sessionRegion();
    expect(await within(region).findByRole("alert")).toBeInTheDocument();
  });

  it("saves the dominant pattern as soon as it is picked, with no Save button to hold", async () => {
    const { calls } = renderPage();
    const alpha = await row("Alpha Song");
    expect(within(alpha).queryByRole("button", { name: "Save" })).toBeNull();
    expect(within(alpha).queryByTestId("hold-ring")).toBeNull();
    expect(within(alpha).getByRole("button", { name: "No clear pattern" })).toHaveAccessibleDescription("Alpha Song");
    expect(within(alpha).getByRole("link", { name: "Inspect in Label screen" })).toHaveAccessibleDescription("Alpha Song");
    const choose = within(alpha).getByRole("button", { name: "Dominant pattern of Alpha Song: Choose pattern" });
    expect(choose).toHaveTextContent("Choose pattern");
    await userEvent.click(choose);
    const picker = await screen.findByRole("dialog", { name: "Dominant pattern of Alpha Song" });
    await userEvent.type(within(picker).getByRole("searchbox", { name: "Search patterns" }), "long");
    await userEvent.click(within(picker).getByRole("button", { name: "lj longjack" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toHaveLength(1);
    });
    const req = argsOf(calls, "session_label_submit")[0]?.["req"] as SessionLabelSubmitDto;
    expect(req).toEqual({ keymode: 7, md5: A, playId: PLAY_A.playId, pattern: "7k.regular.jack.longjack" });
    await waitFor(() => {
      expect(argsOf(calls, "session_plays")).toHaveLength(2);
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("chooses from the pattern grid with the keyboard, saving at once, and leaves on Escape without saving", async () => {
    const { calls } = renderPage();
    const alpha = await row("Alpha Song");
    const choose = within(alpha).getByRole("button", { name: "Dominant pattern of Alpha Song: Choose pattern" });
    await userEvent.click(choose);
    let picker = await screen.findByRole("dialog", { name: "Dominant pattern of Alpha Song" });
    await userEvent.type(within(picker).getByRole("searchbox", { name: "Search patterns" }), "rice jack");
    within(picker).getByRole("button", { name: "mj minijack" }).focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toHaveLength(1);
    });
    expect(argsOf(calls, "session_label_submit")[0]?.["req"]).toEqual({
      keymode: 7,
      md5: A,
      playId: PLAY_A.playId,
      pattern: "7k.regular.jack.minijack",
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    await waitFor(() => {
      expect(choose).toBeEnabled();
    });

    await userEvent.click(choose);
    picker = await screen.findByRole("dialog", { name: "Dominant pattern of Alpha Song" });
    await userEvent.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    expect(argsOf(calls, "session_label_submit")).toHaveLength(1);
  });

  it("disables the answer while a save is on its way", async () => {
    let resolve: (value: { id: string }) => void = () => undefined;
    const { calls } = renderPage({
      sessionLabelSubmit: () =>
        new Promise((r) => {
          resolve = r;
        }),
    });
    const alpha = await row("Alpha Song");
    await userEvent.click(within(alpha).getByRole("button", { name: "No clear pattern" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toHaveLength(1);
    });
    expect(within(alpha).getByRole("button", { name: "No clear pattern" })).toBeDisabled();
    expect(within(alpha).getByRole("button", { name: /^Dominant pattern of Alpha Song:/ })).toBeDisabled();
    await act(async () => {
      resolve({ id: "01NEW" });
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(within(alpha).getByRole("button", { name: "No clear pattern" })).toBeEnabled();
    });
  });

  it("replaces a labelled map's answer with the newly picked one, at once", async () => {
    const { calls } = renderPage();
    const beta = await row("Beta Song");
    await userEvent.click(within(beta).getByRole("button", { name: "Dominant pattern of Beta Song: minijack" }));
    const picker = await screen.findByRole("dialog", { name: "Dominant pattern of Beta Song" });
    await userEvent.type(within(picker).getByRole("searchbox", { name: "Search patterns" }), "stream");
    await userEvent.click(within(picker).getByRole("button", { name: "js jumpstream" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toEqual([
        { req: { keymode: 7, md5: B, playId: PLAY_B.playId, pattern: "7k.regular.stream.jumpstream" } },
      ]);
    });
  });

  it("shows a labelled map's saved pattern on its tile, with the axis glyph and name, pressed in the picker", async () => {
    renderPage();
    const beta = await row("Beta Song");
    const choose = within(beta).getByRole("button", { name: "Dominant pattern of Beta Song: minijack" });
    expect(choose).toHaveTextContent("minijack");
    expect(choose).toHaveTextContent("Jack");
    expect(choose.querySelector("[data-axis-icon='7k.regular.jack']")).not.toBeNull();
    expect(within(beta).getByRole("button", { name: "No clear pattern" })).toHaveAttribute("aria-pressed", "false");

    await userEvent.click(choose);
    const picker = await screen.findByRole("dialog", { name: "Dominant pattern of Beta Song" });

    expect(await within(picker).findByRole("button", { name: "mj minijack" })).toHaveAttribute("aria-pressed", "true");
  });

  it("shows a saved No clear pattern on the tile and presses its button", async () => {
    renderPage({
      sessionPlays: () => ({
        ...SESSION,
        plays: [PLAY_A, { ...PLAY_B, label: { eventId: "01SESSIONB", pattern: null, at: "2026-10-07T11:10:00.000Z" } }],
      }),
    });
    const beta = await row("Beta Song");
    const choose = within(beta).getByRole("button", { name: "Dominant pattern of Beta Song: No clear pattern" });
    expect(choose).toHaveTextContent("No clear pattern");
    expect(choose.querySelector("[data-axis-icon='fallback']")).not.toBeNull();
    expect(within(beta).getByRole("button", { name: "No clear pattern" })).toHaveAttribute("aria-pressed", "true");
  });

  it("loads a map's thumbnail only once its row nears the viewport, over a fallback that keeps the size", async () => {
    const { calls } = renderPage({
      chartBackground: ({ md5 }) => (md5 === A ? { mime: "image/png", base64: "QUJD", width: 1920, height: 1080 } : null),
    });
    const alpha = await row("Alpha Song");
    const beta = await row("Beta Song");
    const alphaThumb = within(alpha).getByTestId("session-thumb");
    expect(observed(alphaThumb)).toBe(true);
    expect(within(alphaThumb).queryByRole("img")).toBeNull();
    expect(within(alphaThumb).getByTestId("session-thumb-fallback")).toBeInTheDocument();
    expect(argsOf(calls, "chart_background")).toHaveLength(0);

    reveal(alphaThumb);

    const image = await within(alphaThumb).findByTestId("session-thumb-image");
    expect(image).toHaveAttribute("src", "data:image/png;base64,QUJD");
    expect(image).toHaveAttribute("alt", "");
    expect(observed(alphaThumb)).toBe(false);
    expect(argsOf(calls, "chart_background")).toEqual([{ md5: A }]);

    const betaThumb = within(beta).getByTestId("session-thumb");
    reveal(betaThumb);
    await waitFor(() => {
      expect(argsOf(calls, "chart_background")).toEqual([{ md5: A }, { md5: B }]);
    });
    expect(within(betaThumb).queryByTestId("session-thumb-image")).toBeNull();
    expect(within(betaThumb).getByTestId("session-thumb-fallback")).toBeInTheDocument();
    expect(betaThumb.className).toEqual(alphaThumb.className);
  });

  it("drops a thumbnail's full-size image once the page is left, as the Label screen does", async () => {
    const { queryClient, router } = renderPage({
      chartBackground: () => ({ mime: "image/png", base64: "QUJD", width: 1920, height: 1080 }),
    });
    const alpha = await row("Alpha Song");
    reveal(within(alpha).getByTestId("session-thumb"));
    await within(alpha).findByTestId("session-thumb-image");
    expect(queryClient.getQueryCache().find({ queryKey: labelKeys.chartBackground(A) })).toBeDefined();

    await userEvent.click(within(alpha).getByRole("link", { name: "Inspect in Label screen" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/label");
    });

    await waitFor(() => {
      expect(queryClient.getQueryCache().find({ queryKey: labelKeys.chartBackground(A) })).toBeUndefined();
    });
  });

  it("keeps the fallback when the thumbnail fails to load", async () => {
    renderPage({ chartBackground: () => mockIpcError("INTERNAL") });
    const thumb = within(await row("Alpha Song")).getByTestId("session-thumb");
    reveal(thumb);
    await waitFor(() => {
      expect(within(thumb).getByTestId("session-thumb-fallback")).toBeInTheDocument();
    });
    expect(within(thumb).queryByTestId("session-thumb-image")).toBeNull();
  });

  it("shows the star rating in osu!'s difficulty colour, as the Label screen's map card", async () => {
    renderPage();
    const alpha = await row("Alpha Song");
    const stars = within(alpha).getByRole("img", { name: "4.21 stars" });
    expect(stars).toHaveTextContent("4.21");
    expect(stars).toHaveStyle({ backgroundColor: difficultyColour(4.21) });
  });

  it("saves No clear pattern as a null pattern at once", async () => {
    const { calls } = renderPage();
    const alpha = await row("Alpha Song");
    await userEvent.click(within(alpha).getByRole("button", { name: "No clear pattern" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toEqual([
        { req: { keymode: 7, md5: A, playId: PLAY_A.playId, pattern: null } },
      ]);
    });
  });

  it("undoes a saved answer by its event id", async () => {
    const { calls } = renderPage();
    const beta = await row("Beta Song");
    await userEvent.click(within(beta).getByRole("button", { name: "Undo" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_undo")).toEqual([{ eventId: "01SESSIONB" }]);
    });
    expect(within(await row("Alpha Song")).queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
  });

  it("words a failed save in the row", async () => {
    renderPage({ sessionLabelSubmit: () => mockIpcError("NOT_FOUND", { md5: A }) });
    const alpha = await row("Alpha Song");
    await userEvent.click(within(alpha).getByRole("button", { name: "No clear pattern" }));
    expect(await within(alpha).findByRole("alert")).toBeInTheDocument();
    expect(within(alpha).getByRole("button", { name: "No clear pattern" })).toBeEnabled();
  });

  it("opens a session map in the Label screen to inspect it", async () => {
    const { router } = renderPage();
    const alpha = await row("Alpha Song");
    await userEvent.click(within(alpha).getByRole("link", { name: "Inspect in Label screen" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/label");
    });
    expect(router.state.location.search).toEqual(expect.objectContaining({ chart: A }));
  });

  it("shows each map as a beatmap card: title, artist, mapper, difficulty and status", async () => {
    renderPage();
    const alpha = await row("Alpha Song");
    expect(within(alpha).getByText("Alpha Song")).toHaveClass("font-bold");
    expect(within(alpha).getByText("by Artist")).toBeInTheDocument();
    expect(alpha).toHaveTextContent("mapped by Mapper");
    expect(within(alpha).getByText("Mapper")).toHaveClass("text-osu-pink");
    expect(within(alpha).getByText("Hard")).toBeInTheDocument();
    expect(within(alpha).getByTestId("session-status")).toHaveTextContent("Pending");
    // The labelled card's tile already shows its answer, so no pill repeats it.
    expect(within(await row("Beta Song")).queryByTestId("session-status")).toBeNull();
  });

  it("counts the chart's gold windows as information beside the answer", async () => {
    renderPage({
      sessionPlays: () => ({ ...SESSION, plays: [{ ...PLAY_A, goldWindows: 3 }, { ...PLAY_B, goldWindows: 1 }] }),
    });
    const alpha = await row("Alpha Song");
    expect(within(alpha).getByText("3 gold windows")).toBeInTheDocument();
    expect(within(alpha).getByTestId("session-status")).toHaveTextContent("Pending");
    expect(within(await row("Beta Song")).getByText("1 gold window")).toBeInTheDocument();
  });

  it("leaves the gold window count out of a map with none", async () => {
    renderPage();
    const alpha = await row("Alpha Song");
    expect(within(alpha).queryByText(/gold window/)).toBeNull();
  });

  it("fades the cover behind the card once its row nears the viewport, under a scrim for the text", async () => {
    renderPage({
      chartBackground: () => ({ mime: "image/png", base64: "QUJD", width: 1920, height: 1080 }),
    });
    const alpha = await row("Alpha Song");
    expect(within(alpha).queryByTestId("session-card-backdrop")).toBeNull();
    reveal(within(alpha).getByTestId("session-thumb"));
    const backdrop = await within(alpha).findByTestId("session-card-backdrop");
    expect(backdrop).toHaveAttribute("src", "data:image/png;base64,QUJD");
    expect(backdrop).toHaveAttribute("alt", "");
    expect(backdrop.closest("[aria-hidden='true']")).not.toBeNull();
    expect(within(alpha).getByTestId("session-card-scrim")).toBeInTheDocument();
  });

  it("explains the coming ranking without showing any data", async () => {
    renderPage();
    const ranking = await screen.findByRole("region", { name: "Ranking" });
    expect(within(ranking).getByText("Coming soon")).toBeInTheDocument();
    expect(within(ranking).queryByRole("list")).not.toBeInTheDocument();
  });

  it("speaks Spanish", async () => {
    await i18n.changeLanguage("es");
    renderPage();
    expect(await screen.findByRole("heading", { level: 1, name: "Progreso del etiquetado" })).toBeInTheDocument();
    const region = await screen.findByRole("region", { name: "Esta sesión" });
    expect(await within(region).findByText("1 etiquetado · 1 pendiente")).toBeInTheDocument();
    expect(within(await screen.findByRole("region", { name: "Totales" })).getByRole("group", { name: "Etiquetado hoy" })).toBeInTheDocument();
  });
});
