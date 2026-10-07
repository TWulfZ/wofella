import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
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
import { LABEL_PROGRESS_PARAMS, LabelProgressPage } from "./LabelProgressPage";

const HOLD_MS = 40;
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

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", NoopResizeObserver);
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
    ...extra,
  });
  const rendered = renderWithRouter(
    <LabelProgressPage keymode={7} now={() => NOW} params={{ ...LABEL_PROGRESS_PARAMS, holdMs: HOLD_MS }} />,
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

async function hold(button: HTMLElement): Promise<void> {
  fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
  await wait(HOLD_MS * 2);
  fireEvent.pointerUp(button, { button: 0, pointerId: 1 });
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

  it("saves one dominant pattern picked from the taxonomy, held like the Label screen's Save", async () => {
    const { calls } = renderPage();
    const alpha = await row("Alpha Song");
    const save = within(alpha).getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();
    expect(save).toHaveAccessibleDescription(expect.stringContaining("Alpha Song"));
    expect(within(alpha).getByRole("button", { name: "No clear pattern" })).toHaveAccessibleDescription("Alpha Song");
    expect(within(alpha).getByRole("link", { name: "Open in Label screen" })).toHaveAccessibleDescription("Alpha Song");
    await userEvent.click(within(alpha).getByRole("button", { name: "Dominant pattern of Alpha Song: Choose pattern" }));
    const search = await screen.findByRole("combobox", { name: "Find a pattern" });
    await userEvent.type(search, "long");
    await userEvent.click(screen.getByRole("option", { name: /longjack/ }));
    expect(screen.queryByRole("combobox", { name: "Find a pattern" })).not.toBeInTheDocument();
    expect(within(alpha).getByRole("button", { name: "Dominant pattern of Alpha Song: longjack" })).toHaveTextContent("longjack");
    await hold(within(alpha).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toHaveLength(1);
    });
    const req = argsOf(calls, "session_label_submit")[0]?.["req"] as SessionLabelSubmitDto;
    expect(req).toEqual({ keymode: 7, md5: A, playId: PLAY_A.playId, pattern: "7k.regular.jack.longjack" });
    await waitFor(() => {
      expect(argsOf(calls, "session_plays")).toHaveLength(2);
    });
  });

  it("saves No clear pattern as a null pattern, exclusive with a picked one", async () => {
    const { calls } = renderPage();
    const alpha = await row("Alpha Song");
    const none = within(alpha).getByRole("button", { name: "No clear pattern" });
    await userEvent.click(none);
    expect(none).toHaveAttribute("aria-pressed", "true");
    await hold(within(alpha).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toHaveLength(1);
    });
    expect((argsOf(calls, "session_label_submit")[0]?.["req"] as SessionLabelSubmitDto).pattern).toBeNull();
  });

  it("does not save on a tap", async () => {
    const { calls } = renderPage();
    const alpha = await row("Alpha Song");
    await userEvent.click(within(alpha).getByRole("button", { name: "No clear pattern" }));
    await userEvent.click(within(alpha).getByRole("button", { name: "Save" }));
    await wait(HOLD_MS * 2);
    expect(argsOf(calls, "session_label_submit")).toHaveLength(0);
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
    await hold(within(alpha).getByRole("button", { name: "Save" }));
    expect(await within(alpha).findByRole("alert")).toBeInTheDocument();
  });

  it("opens a session map in the Label screen", async () => {
    const { router } = renderPage();
    const alpha = await row("Alpha Song");
    await userEvent.click(within(alpha).getByRole("link", { name: "Open in Label screen" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/label");
    });
    expect(router.state.location.search).toEqual(expect.objectContaining({ chart: A }));
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
