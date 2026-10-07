import type { QueryClient } from "@tanstack/react-query";
import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type * as PlayfieldModule from "@/features/playfield";
import { Playfield, type PlayfieldEffects, skinEffectSupport } from "@/features/playfield";
import type {
  AnchorDto,
  ChartDetailsDto,
  ChartTimelineRequestDto,
  ChartWindowDto,
  LabelStatsDto,
  LabelWindowDto,
  MoveWindowRequestDto,
  PatternDefDto,
  PatternExampleDto,
  ResizeWindowRequestDto,
  SampleRequestDto,
  SkinDto,
  SkinEntryDto,
  SkinListDto,
} from "@/ipc/bindings";
import { type CommandHandlers, type MockCall, mockCommands, mockIpcError } from "@/ipc/mocks";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { CHART_HEADER_PARAMS } from "./components/ChartHeader";
import { writeOpenAxes } from "./components/patternGridPrefs";
import { LABEL_SCREEN_PARAMS, LabelScreen, type LabelScreenParams, type LabelScreenProps } from "./LabelScreen";
import { LABEL_PREFS } from "./prefs";
import { skinKeys } from "./queries";

// Playfield passes through, recorded, so its props are observable; skin support is set per test, since what a skin
// supports is the playfield feature's call and is tested there.
vi.mock("@/features/playfield", async (importOriginal) => {
  const actual = await importOriginal<typeof PlayfieldModule>();
  return { ...actual, Playfield: vi.fn(actual.Playfield), skinEffectSupport: vi.fn() };
});

const ALL_EFFECTS_SUPPORTED: Record<keyof PlayfieldEffects, boolean> = {
  percy: true,
  judgements: true,
  combo: true,
  keyPress: true,
  lighting: true,
};

function lastPlayfieldEffects(): PlayfieldEffects | undefined {
  return vi.mocked(Playfield).mock.lastCall?.[0].effects;
}

const TAXONOMY: PatternDefDto[] = [
  { id: "regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "exactly two notes in one column" },
  { id: "regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "three or more notes in one column" },
  { id: "regular.stream.jumpstream", axis: "7k.regular.stream", key: "js", description: "stream with two-note chords" },
];

const ANCHOR_A: AnchorDto = { md5: "a".repeat(32), t0Ms: 1000, t1Ms: 5000, cols: [1, 2, 3, 4, 5, 6, 7] };
const ANCHOR_B: AnchorDto = { md5: "b".repeat(32), t0Ms: 20_000, t1Ms: 24_000, cols: [1, 2, 3, 4, 5, 6, 7] };
const ANCHOR_C: AnchorDto = { md5: "c".repeat(32), t0Ms: 40_000, t1Ms: 44_000, cols: [1, 2, 3, 4, 5, 6, 7] };

function labelWindow(anchor: AnchorDto, title: string): LabelWindowDto {
  return {
    anchor,
    title,
    artist: "Artist",
    version: "Insane",
    creator: "Mapper",
    stars: 4.5,
    level: "dan:7",
    stratum: "dan_07/nps_2",
    played: true,
  };
}

const WINDOW_A = labelWindow(ANCHOR_A, "Alpha Song");
const WINDOW_B = labelWindow(ANCHOR_B, "Beta Song");
const WINDOW_C = labelWindow(ANCHOR_C, "Gamma Song");

const CHART_SPAN = { firstMs: 0, endMs: 90_000 };
const RIGHT_THUMB = "k7.313_right_thumb";
const LEFT_THUMB = "k7.313_left_thumb";

function chartWindow(md5: string, fromMs: number, toMs: number, layoutId: string | null = null): ChartWindowDto {
  return {
    md5,
    keymode: 7,
    fromMs,
    toMs,
    notes: [{ tMs: fromMs, col: 3, endMs: null }],
    timing: [{ tMs: 0, kind: "red", beatLenMs: 300, meter: 4, sv: null }],
    layout: {
      id: layoutId ?? RIGHT_THUMB,
      columns: ["left", "left", "left", "right", "right", "right", "right"].map((hand) => ({
        hand: hand as "left" | "right",
        finger: "index" as const,
      })),
    },
    chartSpan: CHART_SPAN,
    audioFilename: "audio.mp3",
  };
}

const EXAMPLE_MD5 = "0".repeat(32);

// Longjack has no example, so its card keeps the placeholder.
const EXAMPLES: PatternExampleDto[] = [
  { id: "regular.jack.minijack", window: chartWindow(EXAMPLE_MD5, 0, 1500) },
  { id: "regular.stream.jumpstream", window: chartWindow(EXAMPLE_MD5, 0, 1900) },
];

const NO_SKINS: SkinListDto = { skins: [], current: null, maniaSpeed: null, maniaSpeedBpmScale: null };

function skinEntry(folder: string, keymodes: number[], iniMtime = "1"): SkinEntryDto {
  return { folder, name: folder, keymodes, iniMtime };
}

const BROKEN_MIME = "image/x-broken";

/** A skin that draws column 3's note from an image, so a skinned frame always issues a drawImage. */
function skinDto(folder: string, keymode: number, noteMime = "image/png"): SkinDto {
  return {
    folder,
    name: folder,
    version: 2.5,
    config: {
      keys: keymode,
      columnWidth: Array.from({ length: keymode }, () => 42),
      columnSpacing: Array.from({ length: keymode - 1 }, () => 0),
      columnLineWidth: Array.from({ length: keymode + 1 }, () => 2),
      hitPosition: 428,
      lightPosition: 413,
      widthForNoteHeightScale: 42,
      noteBodyStyle: "repeat_bottom",
      judgementLine: true,
      keysUnderNotes: false,
      upsideDown: false,
      barlineHeight: 1.2,
      colours: { column: [], columnLine: null, judgementLine: null, barline: null, hold: null },
    },
    effects: { scorePosition: 300, comboPosition: 111, lightingNWidth: [], lightingLWidth: [], lightColours: [], comboOverlap: 0 },
    images: [{ slot: "note.3", file: 0 }],
    files: [{ mime: noteMime, scale: 1, width: 100, height: 50, base64: "iVBORw0KGgo=" }],
    diagnostics: [],
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
};

const DETAILS: ChartDetailsDto = {
  md5: ANCHOR_A.md5,
  title: "Alpha Song",
  artist: "Artist",
  creator: "Mapper",
  version: "Insane",
  source: "Some Game",
  tags: ["dan", "reform"],
  stars: 4.5,
  od: 8,
  hp: 7.5,
  lengthMs: 154_000,
  bpmMin: 150,
  bpmMax: 180,
  nNotes: 2345,
  nLn: 120,
  setId: 123_456,
  beatmapId: 654_321,
};

/** Short enough to keep the suite fast, long enough that a tap (press and release) never confirms. */
const HOLD_MS = 40;

const TEST_PARAMS: LabelScreenParams = { ...LABEL_SCREEN_PARAMS, holdMs: HOLD_MS };

interface FakeSourceNode {
  started: number;
  stops: number;
  playbackRate: { value: number };
  /** Seconds into the buffer each start asked for. */
  offsets: (number | undefined)[];
}

class FakeAudioContext {
  static created = 0;
  static closed = 0;
  static sources: FakeSourceNode[] = [];
  currentTime = 0;
  state = "running";
  destination = {};
  constructor() {
    FakeAudioContext.created++;
  }
  createBufferSource() {
    const node: FakeSourceNode = { started: 0, stops: 0, playbackRate: { value: 1 }, offsets: [] };
    FakeAudioContext.sources.push(node);
    return {
      buffer: null,
      playbackRate: node.playbackRate,
      onended: null,
      connect: () => undefined,
      disconnect: () => undefined,
      start: (_when?: number, offset?: number) => {
        node.started++;
        node.offsets.push(offset);
      },
      stop: () => {
        node.stops++;
      },
    };
  }
  createGain() {
    return {
      gain: { setValueAtTime: () => undefined, linearRampToValueAtTime: () => undefined },
      connect: () => undefined,
      disconnect: () => undefined,
    };
  }
  async resume() {
    return Promise.resolve();
  }
  async decodeAudioData() {
    return Promise.resolve({ duration: 120 });
  }
  async close() {
    FakeAudioContext.closed++;
    return Promise.resolve();
  }
}

class NoopResizeObserver {
  observe(): void {
    // jsdom does no layout, so the playfield never gets a size here.
  }
  unobserve(): void {
    // See observe.
  }
  disconnect(): void {
    // See observe.
  }
}

beforeEach(() => {
  // Axis cards start collapsed; these tests drive the pattern cards, so every axis starts open.
  writeOpenAxes(TAXONOMY.map((pattern) => pattern.axis));
  FakeAudioContext.created = 0;
  FakeAudioContext.closed = 0;
  FakeAudioContext.sources = [];
  vi.mocked(skinEffectSupport).mockReset().mockReturnValue(ALL_EFFECTS_SUPPORTED);
  vi.stubGlobal("ResizeObserver", NoopResizeObserver);
  vi.stubGlobal("AudioContext", FakeAudioContext);
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    fillStyle: "#000",
    globalAlpha: 1,
    fillRect: () => undefined,
    setTransform: () => undefined,
  } as unknown as RenderingContext);
  return () => {
    vi.unstubAllGlobals();
  };
});

function sampleRequests(calls: MockCall[]): SampleRequestDto[] {
  return calls.filter((c) => c.cmd === "label_sample").map((c) => c.args["req"] as SampleRequestDto);
}

function argsOf(calls: MockCall[], cmd: string): Record<string, unknown>[] {
  return calls.filter((c) => c.cmd === cmd).map((c) => c.args);
}

let queryClient: QueryClient | undefined;

/** Lets queued IPC replies, query updates and effects run, so a test can assert that nothing more was fetched. */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }
}

function renderScreen(
  windows: (LabelWindowDto | null)[] = [WINDOW_A, WINDOW_B],
  extra: CommandHandlers = {},
  props: Partial<LabelScreenProps> = {},
) {
  let eventSeq = 0;
  const calls = mockCommands({
    labelTaxonomy: () => TAXONOMY,
    labelPatternExamples: () => EXAMPLES,
    labelStats: () => STATS,
    labelSample: (args) => windows[(args["req"] as SampleRequestDto).round] ?? null,
    chartWindow: (args) =>
      chartWindow(String(args["md5"]), Number(args["fromMs"]), Number(args["toMs"]), args["layoutId"] as string | null),
    chartAudio: () => ({ mime: "audio/mpeg", base64: "SUQz" }),
    labelSubmit: () => {
      eventSeq++;
      return { id: `01EVENT${eventSeq}` };
    },
    labelUndo: () => null,
    labelRandom: () => null,
    labelNowPlaying: () => null,
    skinList: () => NO_SKINS,
    skinGet: (args) => skinDto(String(args["folder"]), Number(args["keymode"])),
    labelMoveWindow: (args) => {
      const { anchor, t0Ms } = args["req"] as MoveWindowRequestDto;
      const length = anchor.t1Ms - anchor.t0Ms;
      const t0 = Math.min(Math.max(t0Ms, CHART_SPAN.firstMs), CHART_SPAN.endMs - length);
      return { ...anchor, t0Ms: t0, t1Ms: t0 + length };
    },
    labelChartTimeline: (args) => {
      const { buckets } = args["req"] as ChartTimelineRequestDto;
      return {
        firstMs: CHART_SPAN.firstMs,
        endMs: CHART_SPAN.endMs,
        density: Array.from({ length: buckets }, (_, i) => i % 5),
        labelled: [],
      };
    },
    labelResizeWindow: (args) => {
      const { anchor, t0Ms, t1Ms } = args["req"] as ResizeWindowRequestDto;
      return { ...anchor, t0Ms, t1Ms };
    },
    chartBackground: () => null,
    chartDetails: (args) => ({ ...DETAILS, md5: String(args["md5"]) }),
    settingsGetHandLayout: () => RIGHT_THUMB,
    ...extra,
  });
  ({ queryClient } = renderWithRouter(<LabelScreen keymode={7} seed="42" params={TEST_PARAMS} {...props} />, {
    path: "/label",
  }));
  return calls;
}

async function roundLoaded(title = "Alpha Song"): Promise<void> {
  expect(await screen.findByRole("heading", { name: title })).toBeInTheDocument();
}

function answerBar(): HTMLElement {
  return screen.getByRole("region", { name: "Answer" });
}

function inBar(name: string): HTMLElement {
  return within(answerBar()).getByRole("button", { name });
}

function timelineSlider(): HTMLElement {
  return screen.getByRole("slider", { name: "Window position" });
}

/** The timeline shows once the window's chart is drawn, after the header. */
async function timelineShown(): Promise<HTMLElement> {
  return screen.findByRole("slider", { name: "Window position" });
}

function toolbar(): HTMLElement {
  return screen.getByRole("toolbar", { name: "Windows" });
}

function tool(name: string): HTMLElement {
  return within(toolbar()).getByRole("button", { name });
}

/** Pattern names shown as chips in the answer bar, in pick order. */
function chips(): string[] {
  const list = within(answerBar()).queryByRole("list", { name: "Selected patterns" });
  return list === null ? [] : within(list).queryAllByRole("listitem").map((li) => li.dataset["patternId"] ?? "");
}

function submitted(calls: MockCall[]): unknown[] {
  return argsOf(calls, "label_submit").map((a) => a["req"]);
}

function pressed(name: string): string | null {
  return screen.getByRole("button", { name }).getAttribute("aria-pressed");
}

async function clickTool(name: string): Promise<void> {
  await userEvent.click(tool(name));
}

async function wait(ms: number): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

/** Presses a hold-to-confirm button for longer than the hold time, then releases it. */
async function hold(button: HTMLElement): Promise<void> {
  fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
  await wait(HOLD_MS * 2);
  fireEvent.pointerUp(button, { button: 0, pointerId: 1 });
}

async function holdSave(): Promise<void> {
  await hold(inBar("Save"));
}

async function holdSkip(): Promise<void> {
  await hold(inBar("Skip"));
}

/** Holds Enter wherever the focus is, past the hold time. */
async function holdEnter(): Promise<void> {
  await userEvent.keyboard("{Enter>}");
  await wait(HOLD_MS * 2);
  await userEvent.keyboard("{/Enter}");
}

async function saveNoPattern(): Promise<void> {
  await userEvent.click(inBar("No pattern"));
  await holdSave();
}

/** Opens the playback settings flyout, once the player is shown, and returns its panel. */
async function openSettings(): Promise<HTMLElement> {
  const open = screen.queryByRole("dialog", { name: "Playback settings" });
  if (open !== null) {
    return open;
  }
  await userEvent.click(await screen.findByRole("button", { name: "Playback settings" }));
  return screen.getByRole("dialog", { name: "Playback settings" });
}

async function setting(
  role: "slider" | "combobox" | "spinbutton" | "checkbox" | "button",
  name: string,
): Promise<HTMLElement> {
  return within(await openSettings()).findByRole(role, { name });
}

async function playingSource(): Promise<FakeSourceNode> {
  await userEvent.click(await screen.findByRole("button", { name: "Play" }));
  await waitFor(() => {
    // The playing iteration and the next one, scheduled ahead.
    expect(FakeAudioContext.sources.map((s) => s.started)).toEqual([1, 1]);
  });
  const [source] = FakeAudioContext.sources;
  if (source === undefined) {
    throw new Error("no source node");
  }
  return source;
}

function never<T>(): Promise<T> {
  return new Promise<T>(() => undefined);
}

describe("LabelScreen", () => {
  it("loads a round blind and mounts the playfield canvas", async () => {
    const calls = renderScreen();
    await roundLoaded();

    expect(sampleRequests(calls)).toEqual([
      { keymode: 7, seed: "42", round: 0, windowMs: null, scale: null, levelMin: null, levelMax: null, exclude: [] },
    ]);
    await waitFor(() => {
      expect(argsOf(calls, "chart_window")).toEqual([{ md5: ANCHOR_A.md5, fromMs: 1000, toMs: 5000, layoutId: RIGHT_THUMB }]);
    });
    expect(await screen.findByTestId("playfield")).toContainHTML("<canvas");
    expect(screen.getByText("00:01.000–00:05.000")).toBeInTheDocument();
    expect(screen.getByText("Round 1")).toBeInTheDocument();
    expect(screen.getByText("Insane")).toBeInTheDocument();
    expect(screen.getByText("Played")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "js jumpstream" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByText("Gold set: 37")).toBeInTheDocument();
  });

  it("shows a card per pattern in the Patterns region, drawing its synthetic example", async () => {
    const calls = renderScreen();
    await roundLoaded();

    const patterns = screen.getByRole("region", { name: "Patterns" });
    const jumpstream = await within(patterns).findByRole("button", { name: "js jumpstream" });
    await waitFor(() => {
      expect(jumpstream.querySelector("canvas")).not.toBeNull();
    });
    expect(within(patterns).getByRole("button", { name: "mj minijack" }).querySelector("canvas")).not.toBeNull();
    expect(within(patterns).getByRole("button", { name: "lj longjack" }).querySelector("canvas")).toBeNull();
    expect(argsOf(calls, "label_pattern_examples")).toEqual([{ keymode: 7, layoutId: RIGHT_THUMB }]);
  });

  it("has no free-text answer: no Answer textbox, and printable keys outside a field run nothing", async () => {
    const calls = renderScreen();
    await roundLoaded();

    expect(screen.queryByRole("textbox", { name: "Answer" })).toBeNull();
    await userEvent.keyboard("sux");
    expect(screen.getByText("Skipped: 0")).toBeInTheDocument();
    expect(chips()).toEqual([]);
    expect(pressed("No pattern")).toBe("false");
    expect(argsOf(calls, "label_undo")).toEqual([]);
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();
  });

  it("filters the cards from the pattern search without touching the answer", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "mj minijack" }));

    await userEvent.type(screen.getByRole("searchbox", { name: "Search patterns" }), "jump");

    expect(screen.getByRole("button", { name: "js jumpstream" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "mj minijack" })).toBeNull();
    expect(screen.queryByRole("button", { name: "lj longjack" })).toBeNull();
    expect(chips()).toEqual(["regular.jack.minijack"]);
  });

  it("still labels with the cards when the pattern examples fail", async () => {
    const calls = renderScreen([WINDOW_A, WINDOW_B], {
      labelPatternExamples: () => mockIpcError("INTERNAL"),
    });
    await roundLoaded();
    await waitFor(() => {
      expect(argsOf(calls, "label_pattern_examples")).toEqual([{ keymode: 7, layoutId: RIGHT_THUMB }]);
    });
    await settle();

    const jumpstream = screen.getByRole("button", { name: "js jumpstream" });
    expect(jumpstream.querySelector("canvas")).toBeNull();
    await userEvent.click(jumpstream);
    expect(chips()).toEqual(["regular.stream.jumpstream"]);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("submits the patterns picked on the cards and the flags on Enter, as taxonomy ids", async () => {
    const calls = renderScreen();
    await roundLoaded();

    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    await userEvent.click(screen.getByRole("button", { name: "mj minijack" }));
    await userEvent.click(inBar("Mixed"));
    expect(chips()).toEqual(["regular.stream.jumpstream", "regular.jack.minijack"]);
    expect(pressed("js jumpstream")).toBe("true");
    expect(pressed("Mixed")).toBe("true");
    await holdEnter();

    await roundLoaded("Beta Song");
    expect(submitted(calls)).toEqual([
      {
        anchor: ANCHOR_A,
        patterns: ["regular.stream.jumpstream", "regular.jack.minijack"],
        noPattern: false,
        mixed: true,
        unsure: false,
        thumbPref: null,
      },
    ]);
    expect(argsOf(calls, "label_resolve_patterns")).toEqual([]);
    expect(sampleRequests(calls)[1]).toMatchObject({ round: 1, exclude: [ANCHOR_A] });
    expect(screen.getByText("Labelled: 1")).toBeInTheDocument();
    expect(chips()).toEqual([]);
    expect(pressed("Mixed")).toBe("false");
  });

  it("marks the labelling progress stale after a save, so the counters popover shows today's count", async () => {
    renderScreen();
    await roundLoaded();
    const progressKey = ["labels", "progress", 7, 0];
    queryClient?.setQueryData(progressKey, { goldTotal: 0 });
    await saveNoPattern();
    await roundLoaded("Beta Song");
    expect(queryClient?.getQueryState(progressKey)?.isInvalidated).toBe(true);
  });

  it("removes a chip with its × and toggles it off from its card", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    await userEvent.click(screen.getByRole("button", { name: "lj longjack" }));

    await userEvent.click(inBar("Remove jumpstream"));
    expect(chips()).toEqual(["regular.jack.longjack"]);
    expect(pressed("js jumpstream")).toBe("false");
    await userEvent.click(screen.getByRole("button", { name: "lj longjack" }));
    expect(chips()).toEqual([]);
  });

  it("adds patterns from the + picker: type to filter, Enter adds, Esc closes, then Enter saves", async () => {
    const calls = renderScreen();
    await roundLoaded();

    await userEvent.click(inBar("Add pattern"));
    const find = await screen.findByRole("combobox", { name: "Find a pattern" });
    expect(find).toHaveFocus();
    await userEvent.keyboard("jump");
    const list = screen.getByRole("listbox", { name: "Taxonomy patterns" });
    expect(within(list).getAllByRole("option").map((o) => o.textContent)).toEqual([
      expect.stringContaining("jumpstream"),
    ]);
    await userEvent.keyboard("{Enter}");
    expect(chips()).toEqual(["regular.stream.jumpstream"]);
    expect(find).toHaveValue("");
    expect(screen.getByRole("option", { name: "js jumpstream" })).toHaveAttribute("aria-checked", "true");

    await userEvent.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByRole("combobox", { name: "Find a pattern" })).toBeNull();
    });
    expect(chips()).toEqual(["regular.stream.jumpstream"]);
    await holdEnter();
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toMatchObject([{ patterns: ["regular.stream.jumpstream"], noPattern: false }]);
  });

  it("only adds from the + picker: picking a chosen pattern again keeps it", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.click(inBar("Add pattern"));
    await screen.findByRole("combobox", { name: "Find a pattern" });

    await userEvent.keyboard("jump{Enter}jump{Enter}");
    expect(chips()).toEqual(["regular.stream.jumpstream"]);
    await userEvent.click(screen.getByRole("option", { name: "js jumpstream" }));
    expect(chips()).toEqual(["regular.stream.jumpstream"]);
    expect(pressed("js jumpstream")).toBe("true");
  });

  it("moves through the + picker with the arrow keys", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.click(inBar("Add pattern"));
    const find = await screen.findByRole("combobox", { name: "Find a pattern" });

    expect(screen.getByRole("option", { name: "mj minijack" })).toHaveAttribute("aria-selected", "true");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowUp}");
    const longjack = screen.getByRole("option", { name: "lj longjack" });
    expect(longjack).toHaveAttribute("aria-selected", "true");
    expect(find).toHaveAttribute("aria-activedescendant", longjack.id);
    await userEvent.keyboard("{Enter}");
    expect(chips()).toEqual(["regular.jack.longjack"]);
    expect(pressed("lj longjack")).toBe("true");
  });

  it("makes No pattern exclusive with the patterns and stores it without patterns", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    await userEvent.click(inBar("No pattern"));
    expect(chips()).toEqual([]);
    expect(pressed("No pattern")).toBe("true");
    await userEvent.click(screen.getByRole("button", { name: "mj minijack" }));
    expect(pressed("No pattern")).toBe("false");
    await userEvent.click(inBar("No pattern"));

    await holdSave();

    await roundLoaded("Beta Song");
    expect(submitted(calls)).toEqual([
      { anchor: ANCHOR_A, patterns: [], noPattern: true, mixed: false, unsure: false, thumbPref: null },
    ]);
  });

  it("keeps Save disabled and Enter inert until the answer has a pattern or No pattern", async () => {
    const calls = renderScreen();
    await roundLoaded();
    expect(inBar("Save")).toBeDisabled();
    await userEvent.click(inBar("Unsure"));
    await holdEnter();
    await settle();
    expect(submitted(calls)).toEqual([]);
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();
  });

  it("pins the answer bar to the bottom of the scrolling pattern panel", async () => {
    renderScreen();
    await roundLoaded();
    const bar = answerBar();
    expect(bar).toHaveClass("sticky", "bottom-0");
    const list = screen.getByTestId("pattern-scroll");
    expect(list).toHaveClass("overflow-y-auto");
    expect(bar.parentElement?.lastElementChild).toBe(bar);
    expect(list.lastElementChild).toBe(bar.parentElement);
    expect(within(list).getByRole("region", { name: "Patterns" })).toBeInTheDocument();
  });

  it("scrolls the map card with the panel, edge to edge, ahead of the pattern list", async () => {
    renderScreen();
    await roundLoaded();
    const panel = screen.getByTestId("pattern-panel");
    const list = screen.getByTestId("pattern-scroll");
    const header = screen.getByRole("heading", { name: "Alpha Song" }).closest("header");
    expect(list.parentElement).toBe(panel);
    expect(list).toContainElement(header);
    expect(header?.parentElement).toBe(list);
    for (const el of [panel, list]) {
      expect(el.className).not.toMatch(/(^|\s)(p|px|pl|pr|gap)-\S+/);
    }
    expect(header).toHaveClass("w-full", "border-b");
    const patterns = within(list).getByRole("region", { name: "Patterns" });
    expect((header?.compareDocumentPosition(patterns) ?? 0) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("keeps a focused card clear of the compact title bar the card collapses into", async () => {
    renderScreen();
    await roundLoaded();
    expect(screen.getByTestId("pattern-scroll").style.scrollPaddingTop).toBe(`${String(CHART_HEADER_PARAMS.compactBarPx + 8)}px`);
  });

  it("paints the card's background under the panel's scrollbar, scrolling with it", async () => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(private readonly onResize: ResizeObserverCallback) {}
        observe(target: Element): void {
          if (target.tagName === "HEADER") {
            this.onResize(
              [{ target, borderBoxSize: [{ blockSize: 212, inlineSize: 300 }] } as unknown as ResizeObserverEntry],
              this,
            );
          }
        }
        unobserve(): void {
          // Nothing is held.
        }
        disconnect(): void {
          // Nothing is held.
        }
      },
    );
    renderScreen([WINDOW_A, WINDOW_B], {
      chartBackground: () => ({ mime: "image/png", base64: "iVBORw0KGgo=", width: 1920, height: 1080 }),
    });
    await roundLoaded();
    const list = screen.getByTestId("pattern-scroll");
    await waitFor(() => {
      expect(list.style.backgroundImage).toContain("data:image/png;base64,iVBORw0KGgo=");
    });
    expect(list.style.backgroundAttachment).toBe("local");
    expect(list.style.backgroundRepeat).toBe("no-repeat");
    // The scrim over the image, both sized to the card.
    expect(list.style.backgroundImage).toMatch(/^linear-gradient\(.*var\(--background\)\), url\(/);
    expect(list.style.backgroundSize).toBe("100% 212px, 100% 212px");
  });

  it("pads the panel's scrolling by the answer bar's height, so a card focused with Tab never hides under it", async () => {
    const barPx = 180;
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(private readonly onResize: ResizeObserverCallback) {}
        observe(target: Element): void {
          if (target.getAttribute("aria-label") === "Answer") {
            this.onResize(
              [{ target, borderBoxSize: [{ blockSize: barPx, inlineSize: 300 }] } as unknown as ResizeObserverEntry],
              this,
            );
          }
        }
        unobserve(): void {
          // Nothing is held.
        }
        disconnect(): void {
          // Nothing is held.
        }
      },
    );
    renderScreen();
    await roundLoaded();
    expect(screen.getByTestId("pattern-scroll").style.scrollPaddingBottom).toBe("188px");
  });

  it("skips only on a held Skip: nothing stored, counted, and the next sample excludes the shown window", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(inBar("Skip"));
    await settle();
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();

    await holdSkip();

    await roundLoaded("Beta Song");
    expect(submitted(calls)).toEqual([]);
    expect(sampleRequests(calls)[1]).toMatchObject({ seed: "42", round: 1, exclude: [ANCHOR_A] });
    expect(screen.getByText("Skipped: 1")).toBeInTheDocument();
  });

  it("walks the session history with Previous and Next, sampling only past the newest window", async () => {
    const calls = renderScreen([WINDOW_A, WINDOW_B, WINDOW_C]);
    await roundLoaded();
    expect(tool("Previous")).toHaveAttribute("aria-disabled", "true");
    expect(tool("Previous")).toHaveAccessibleDescription("This is the first window of the session.");

    await clickTool("Next");
    await roundLoaded("Beta Song");
    expect(sampleRequests(calls)).toHaveLength(2);
    await clickTool("Previous");
    await roundLoaded("Alpha Song");
    await clickTool("Next");
    await roundLoaded("Beta Song");
    expect(sampleRequests(calls)).toHaveLength(2);
    expect(screen.getByText("Skipped: 0")).toBeInTheDocument();

    await clickTool("Next");
    await roundLoaded("Gamma Song");
    expect(sampleRequests(calls)[2]).toMatchObject({ round: 2, exclude: [ANCHOR_A, ANCHOR_B] });
  });

  it("does nothing on a disabled Previous", async () => {
    renderScreen();
    await roundLoaded();
    await clickTool("Previous");
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();
  });

  it("shows a saved window read-only when revisited and undoes it, letting it be labelled again", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "lj longjack" }));
    await userEvent.click(inBar("Right thumb"));
    await holdSave();
    await roundLoaded("Beta Song");

    await clickTool("Previous");
    await roundLoaded("Alpha Song");
    expect(within(answerBar()).getByText("Saved")).toBeInTheDocument();
    expect(chips()).toEqual(["regular.jack.longjack"]);
    expect(within(answerBar()).queryByRole("button", { name: "Remove longjack" })).toBeNull();
    expect(pressed("Right thumb")).toBe("true");
    expect(inBar("Right thumb")).toBeDisabled();
    expect(within(answerBar()).queryByRole("button", { name: "Save" })).toBeNull();
    expect(inBar("Skip")).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    expect(chips()).toEqual(["regular.jack.longjack"]);

    await userEvent.click(inBar("Undo"));
    await waitFor(() => {
      expect(argsOf(calls, "label_undo")).toEqual([{ eventId: "01EVENT1" }]);
    });
    expect(await screen.findByText("Undone: 1")).toBeInTheDocument();
    expect(screen.getByText("Last label undone.")).toBeInTheDocument();
    expect(within(answerBar()).queryByText("Saved")).toBeNull();
    expect(chips()).toEqual([]);

    await saveNoPattern();
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toHaveLength(2);
    expect(sampleRequests(calls)).toHaveLength(2);
  });

  it("only offers Undo on the newest saved window", async () => {
    const calls = renderScreen([WINDOW_A, WINDOW_B, WINDOW_C]);
    await roundLoaded();
    await saveNoPattern();
    await roundLoaded("Beta Song");
    await saveNoPattern();
    await roundLoaded("Gamma Song");

    await clickTool("Previous");
    await roundLoaded("Beta Song");
    expect(inBar("Undo")).toHaveAttribute("aria-disabled", "false");
    await clickTool("Previous");
    await roundLoaded("Alpha Song");
    expect(inBar("Undo")).toHaveAttribute("aria-disabled", "true");
    expect(inBar("Undo")).toHaveAccessibleDescription("Only the newest saved label can be undone.");
    await userEvent.click(inBar("Undo"));
    expect(argsOf(calls, "label_undo")).toEqual([]);
  });

  it("keeps the undo stack when an undo fails, so it can be retried", async () => {
    let undoCalls = 0;
    const calls = renderScreen(undefined, {
      labelUndo: () => {
        undoCalls++;
        return undoCalls === 1 ? mockIpcError("INTERNAL") : null;
      },
    });
    await roundLoaded();
    await saveNoPattern();
    await roundLoaded("Beta Song");
    await clickTool("Previous");
    await roundLoaded("Alpha Song");

    await userEvent.click(inBar("Undo"));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("Undone: 0")).toBeInTheDocument();

    await userEvent.click(inBar("Undo"));
    expect(await screen.findByText("Undone: 1")).toBeInTheDocument();
    expect(argsOf(calls, "label_undo")).toEqual([{ eventId: "01EVENT1" }, { eventId: "01EVENT1" }]);
  });

  it("lets a skipped window be labelled when revisited, without counting it twice", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await holdSkip();
    await roundLoaded("Beta Song");
    await clickTool("Previous");
    await roundLoaded("Alpha Song");
    expect(within(answerBar()).getByText("Skipped")).toBeInTheDocument();
    await holdSkip();
    await roundLoaded("Beta Song");
    expect(screen.getByText("Skipped: 1")).toBeInTheDocument();

    await clickTool("Previous");
    await roundLoaded("Alpha Song");
    await saveNoPattern();
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toMatchObject([{ anchor: ANCHOR_A, noPattern: true }]);
  });

  it("picks a random window outside the stratified plan and says when none is left", async () => {
    let randomCalls = 0;
    const calls = renderScreen(undefined, {
      labelRandom: () => {
        randomCalls++;
        return randomCalls === 1 ? WINDOW_C : null;
      },
    });
    await roundLoaded();

    await clickTool("Random");
    await roundLoaded("Gamma Song");
    expect(screen.getByText("Random pick")).toBeInTheDocument();
    expect(argsOf(calls, "label_random")).toEqual([
      { req: { keymode: 7, seed: "42", round: 0, windowMs: null, exclude: [ANCHOR_A] } },
    ]);
    expect(sampleRequests(calls)).toHaveLength(1);

    await clickTool("Random");
    expect(await screen.findByText("No chart has a free window left for a random pick.")).toBeInTheDocument();
    expect(argsOf(calls, "label_random")[1]).toEqual({
      req: { keymode: 7, seed: "42", round: 1, windowMs: null, exclude: [ANCHOR_A, ANCHOR_C] },
    });
    expect(screen.getByRole("heading", { name: "Gamma Song" })).toBeInTheDocument();

    await clickTool("Next");
    await roundLoaded("Beta Song");
    expect(sampleRequests(calls)[1]).toMatchObject({ round: 1, exclude: [ANCHOR_A, ANCHOR_C] });
  });

  it("words a failed random pick", async () => {
    renderScreen(undefined, { labelRandom: () => mockIpcError("INTERNAL") });
    await roundLoaded();
    await clickTool("Random");
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();
  });

  it("opens the chart osu! is playing and names where it came from", async () => {
    const calls = renderScreen(undefined, {
      labelNowPlaying: () => ({ window: WINDOW_C, source: "osuWindow" }),
    });
    await roundLoaded();

    await clickTool("Now playing");
    await roundLoaded("Gamma Song");
    expect(screen.getByText("From osu!")).toBeInTheDocument();
    expect(argsOf(calls, "label_now_playing")).toEqual([{ req: { keymode: 7, exclude: [ANCHOR_A] } }]);

    await clickTool("Now playing");
    await settle();
    await clickTool("Previous");
    await roundLoaded("Alpha Song");
    await clickTool("Next");
    await roundLoaded("Gamma Song");
    await clickTool("Next");
    await roundLoaded("Beta Song");
    expect(sampleRequests(calls)[1]).toMatchObject({ round: 1, exclude: [ANCHOR_A, ANCHOR_C] });
  });

  it("opens a chart handed over from the session list before sampling the plan", async () => {
    const calls = renderScreen(undefined, { labelWindowAt: () => WINDOW_C }, { openChart: ANCHOR_C.md5 });
    await roundLoaded("Gamma Song");
    expect(screen.getByText("Played this session")).toBeInTheDocument();
    expect(argsOf(calls, "label_window_at")).toEqual([
      { req: { keymode: 7, md5: ANCHOR_C.md5, seed: "42", windowMs: null, exclude: [] } },
    ]);
    expect(sampleRequests(calls)).toHaveLength(0);

    await clickTool("Next");
    await roundLoaded("Alpha Song");
    expect(sampleRequests(calls)[0]).toMatchObject({ round: 0, exclude: [ANCHOR_C] });
  });

  it("words a chart that cannot be opened and samples the plan instead", async () => {
    const calls = renderScreen(undefined, { labelWindowAt: () => mockIpcError("CONFLICT") }, { openChart: ANCHOR_C.md5 });
    await roundLoaded("Alpha Song");
    expect(argsOf(calls, "label_window_at")).toHaveLength(1);
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("puts the progress summary it is handed in the counters popover", async () => {
    renderScreen(undefined, {}, { countersDetails: <p>Progress summary</p> });
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "Labelling progress" }));
    expect(await screen.findByText("Progress summary")).toBeInTheDocument();
  });

  it("still peeks the counters popover on keyboard focus after the mouse used its trigger", async () => {
    renderScreen(undefined, {}, { countersDetails: <p>Progress summary</p> });
    await roundLoaded();
    const trigger = screen.getByRole("button", { name: "Labelling progress" });
    // A press that ends off the trigger, so no click follows; the screen keeps focus where it was.
    fireEvent.pointerDown(trigger, { button: 0, pointerId: 1 });
    fireEvent.mouseDown(trigger, { button: 0 });
    fireEvent.pointerUp(document.body, { button: 0, pointerId: 1 });
    expect(trigger).not.toHaveFocus();
    act(() => {
      trigger.focus();
    });
    expect(await screen.findByText("Progress summary")).toBeInTheDocument();
    expect(trigger).toHaveAttribute("aria-expanded", "true");
  });

  it("sends the shown windows with Now playing, so a skipped one is not offered again", async () => {
    let nowPlayingCalls = 0;
    const calls = renderScreen(undefined, {
      labelNowPlaying: () => {
        nowPlayingCalls++;
        return nowPlayingCalls === 1 ? { window: WINDOW_C, source: "osuWindow" } : null;
      },
    });
    await roundLoaded();

    await clickTool("Now playing");
    await roundLoaded("Gamma Song");
    await holdSkip();
    await roundLoaded("Beta Song");
    await clickTool("Now playing");

    expect(
      await screen.findByText("osu! is not playing a chart and none of your replays was found."),
    ).toBeInTheDocument();
    expect(argsOf(calls, "label_now_playing")).toEqual([
      { req: { keymode: 7, exclude: [ANCHOR_A] } },
      { req: { keymode: 7, exclude: [ANCHOR_A, ANCHOR_C, ANCHOR_B] } },
    ]);
  });

  it("falls back to the last replay, and says so when nothing is playing or was played", async () => {
    let nowPlayingCalls = 0;
    renderScreen(undefined, {
      labelNowPlaying: () => {
        nowPlayingCalls++;
        return nowPlayingCalls === 1 ? null : { window: WINDOW_C, source: "lastReplay" };
      },
    });
    await roundLoaded();

    await clickTool("Now playing");
    expect(
      await screen.findByText("osu! is not playing a chart and none of your replays was found."),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();

    await clickTool("Now playing");
    await roundLoaded("Gamma Song");
    expect(screen.getByText("Last played")).toBeInTheDocument();
  });

  it("toggles play with Space outside a text field or button", async () => {
    renderScreen();
    await roundLoaded();
    await screen.findByRole("button", { name: "Play" });
    await userEvent.keyboard(" ");
    expect(await screen.findByRole("button", { name: "Pause" })).toBeInTheDocument();
  });

  it("toggles play once while Space is held", async () => {
    renderScreen();
    await roundLoaded();
    await screen.findByRole("button", { name: "Play" });
    await userEvent.keyboard("[Space>2]");
    expect(await screen.findByRole("button", { name: "Pause" })).toBeInTheDocument();
  });

  it("keeps focus off clicked buttons and the playfield, so Enter still saves", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    await userEvent.click(await screen.findByRole("button", { name: "Play" }));
    await userEvent.click(screen.getByTestId("playfield"));
    expect(document.body).toHaveFocus();

    await holdEnter();
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toMatchObject([{ patterns: ["regular.stream.jumpstream"] }]);
  });

  it("lets Space toggle a skin effect checkbox in the settings instead of playing", async () => {
    renderScreen();
    await roundLoaded();
    const combo = await setting("checkbox", "Show combo");
    combo.focus();
    await userEvent.keyboard(" ");
    expect(combo).toBeChecked();
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
  });

  it("drops an Enter hold when the window loses focus, and a later Enter hold still saves", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(inBar("No pattern"));

    fireEvent.keyDown(document.body, { key: "Enter" });
    await wait(HOLD_MS / 2);
    fireEvent.blur(window);
    await wait(HOLD_MS * 2);
    expect(submitted(calls)).toEqual([]);

    // The keyup went to another app, so this press arrives with no release in between.
    fireEvent.keyDown(document.body, { key: "Enter" });
    await wait(HOLD_MS * 2);
    fireEvent.keyUp(document.body, { key: "Enter" });
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toHaveLength(1);
  });

  it("drops a pointer hold on Save when the window loses focus", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(inBar("No pattern"));

    fireEvent.pointerDown(inBar("Save"), { button: 0, pointerId: 1 });
    await wait(HOLD_MS / 2);
    fireEvent.blur(window);
    await wait(HOLD_MS * 2);

    expect(submitted(calls)).toEqual([]);
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();
  });

  it("keeps Enter for saving after a chip click shows its pattern card", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "mj minijack" }));
    await userEvent.click(within(answerBar()).getByRole("button", { name: "Show minijack in the pattern panel" }));
    expect(screen.getByRole("button", { name: "mj minijack" })).toHaveAttribute("data-highlighted", "true");

    await holdEnter();

    await roundLoaded("Beta Song");
    expect(submitted(calls)).toMatchObject([{ patterns: ["regular.jack.minijack"] }]);
  });

  it("stores one label while Enter is held across the round change", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(inBar("No pattern"));
    await userEvent.keyboard("{Enter>20}");
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toHaveLength(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("drops a second Enter hold that starts while the save is in flight", async () => {
    const calls = renderScreen(undefined, {
      labelSubmit: () => {
        // Many microtask hops let the first submit settle, while React's render waits for a macrotask.
        void (async () => {
          for (let i = 0; i < 1000; i++) {
            await Promise.resolve();
          }
          fireEvent.keyDown(document.body, { key: "Enter" });
        })();
        return { id: "01EVENT1" };
      },
    });
    await roundLoaded();
    await userEvent.click(inBar("No pattern"));
    fireEvent.keyDown(document.body, { key: "Enter" });

    await roundLoaded("Beta Song");
    await wait(HOLD_MS * 3);
    fireEvent.keyUp(document.body, { key: "Enter" });
    expect(submitted(calls)).toHaveLength(1);
    expect(screen.getByText("Labelled: 1")).toBeInTheDocument();
  });

  it("saves on a held Enter only, a tap stores nothing", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(inBar("No pattern"));
    await userEvent.keyboard("{Enter}");
    await wait(HOLD_MS * 2);
    expect(submitted(calls)).toEqual([]);

    await holdEnter();
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toHaveLength(1);
  });

  it("clears the answer with Escape or the Clear button", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    await userEvent.click(inBar("Mixed"));
    await userEvent.keyboard("{Escape}");
    expect(chips()).toEqual([]);
    expect(pressed("js jumpstream")).toBe("false");
    expect(pressed("Mixed")).toBe("false");

    await userEvent.click(inBar("No pattern"));
    await userEvent.click(inBar("Clear"));
    expect(pressed("No pattern")).toBe("false");
  });

  it("keeps the answer and flags across a timeline move and resets them on the next window", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    await userEvent.click(inBar("Mixed"));

    (await timelineShown()).focus();
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    expect(await screen.findByText("00:03.000–00:07.000")).toBeInTheDocument();
    expect(chips()).toEqual(["regular.stream.jumpstream"]);
    expect(pressed("Mixed")).toBe("true");

    await holdSave();
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toMatchObject([{ anchor: { ...ANCHOR_A, t0Ms: 3000, t1Ms: 7000 }, mixed: true }]);
    expect(pressed("Mixed")).toBe("false");
  });

  it("stores the flags set with the buttons", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(inBar("Unsure"));
    await userEvent.click(inBar("Left thumb"));
    await userEvent.click(inBar("Right thumb"));
    expect(pressed("Left thumb")).toBe("false");
    await userEvent.click(screen.getByRole("button", { name: "lj longjack" }));
    await holdEnter();

    await roundLoaded("Beta Song");
    expect(submitted(calls)).toEqual([
      {
        anchor: ANCHOR_A,
        patterns: ["regular.jack.longjack"],
        noPattern: false,
        mixed: false,
        unsure: true,
        thumbPref: "right",
      },
    ]);
  });

  it("has no widen, narrow or shift window buttons any more", async () => {
    renderScreen();
    await roundLoaded();
    await screen.findByRole("button", { name: "Play" });
    expect(screen.queryByRole("group", { name: "Window" })).toBeNull();
    for (const name of ["Widen window", "Narrow window", "Shift window earlier", "Shift window later"]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
  });

  it("moves the window from the timeline keyboard, reloading the playfield and audio section without a new sample", async () => {
    const calls = renderScreen();
    await roundLoaded();
    const slider = await timelineShown();
    expect(slider).toHaveAttribute("aria-valuenow", "1000");
    expect(slider).toHaveAttribute("aria-valuetext", "00:01 to 00:05");

    slider.focus();
    await userEvent.keyboard("{PageUp}");

    expect(await screen.findByText("00:06.000–00:10.000")).toBeInTheDocument();
    expect(argsOf(calls, "label_move_window")).toEqual([{ req: { anchor: ANCHOR_A, t0Ms: 6000 } }]);
    await waitFor(() => {
      expect(argsOf(calls, "chart_window").at(-1)).toEqual({
        md5: ANCHOR_A.md5,
        fromMs: 6000,
        toMs: 10_000,
        layoutId: RIGHT_THUMB,
      });
    });
    expect(timelineSlider()).toHaveAttribute("aria-valuenow", "6000");
    expect(sampleRequests(calls)).toHaveLength(1);
    expect(argsOf(calls, "chart_audio")).toHaveLength(1);
    expect(argsOf(calls, "label_chart_timeline")).toEqual([{ req: { keymode: 7, md5: ANCHOR_A.md5, buckets: 240 } }]);
  });

  it("moves the window to a click on the timeline, centred on it, committed on release", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await timelineShown();
    await waitFor(() => {
      expect(argsOf(calls, "label_chart_timeline")).toHaveLength(1);
    });
    await settle();
    // 900 px for the 90 s chart: 1 px is 100 ms.
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(DOMRect.fromRect({ x: 0, width: 900 }));
    const track = screen.getByTestId("timeline-track");
    fireEvent.pointerDown(track, { clientX: 450, button: 0, pointerId: 1 });
    expect(argsOf(calls, "label_move_window")).toEqual([]);
    fireEvent.pointerUp(window, { clientX: 450, pointerId: 1 });

    expect(await screen.findByText("00:43.000–00:47.000")).toBeInTheDocument();
    expect(argsOf(calls, "label_move_window")).toEqual([{ req: { anchor: ANCHOR_A, t0Ms: 43_000 } }]);
  });

  it("keeps the window and words the error when a move fails", async () => {
    renderScreen([WINDOW_A, WINDOW_B], { labelMoveWindow: () => mockIpcError("NOT_FOUND") });
    await roundLoaded();
    (await timelineShown()).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(await within(answerBar()).findByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("00:01.000–00:05.000")).toBeInTheDocument();
    expect(timelineSlider()).toHaveAttribute("aria-valuenow", "1000");
  });

  it("shades the own labelled windows and refetches the timeline after a save and an undo", async () => {
    const labelled: AnchorDto[] = [];
    const calls = renderScreen([WINDOW_A, WINDOW_B], {
      labelSubmit: (args) => {
        labelled.push((args["req"] as { anchor: AnchorDto }).anchor);
        return { id: "01EVENT1" };
      },
      labelUndo: () => {
        labelled.pop();
        return null;
      },
      labelChartTimeline: (args) => {
        const { md5, buckets } = args["req"] as ChartTimelineRequestDto;
        return {
          firstMs: CHART_SPAN.firstMs,
          endMs: CHART_SPAN.endMs,
          density: Array.from({ length: buckets }, () => 1),
          labelled: labelled.filter((a) => a.md5 === md5).map(({ t0Ms, t1Ms }) => ({ t0Ms, t1Ms })),
        };
      },
    });
    const timelineCallsFor = (md5: string) =>
      argsOf(calls, "label_chart_timeline").filter((a) => (a["req"] as ChartTimelineRequestDto).md5 === md5).length;
    await roundLoaded();
    expect(await screen.findByText("0 labelled windows on this chart")).toBeInTheDocument();
    expect(timelineCallsFor(ANCHOR_A.md5)).toBe(1);

    await saveNoPattern();
    await roundLoaded("Beta Song");
    await clickTool("Previous");
    await roundLoaded();
    await timelineShown();
    expect(await screen.findByText("1 labelled window on this chart")).toBeInTheDocument();
    expect(timelineCallsFor(ANCHOR_A.md5)).toBe(2);
    expect(timelineSlider()).toHaveAttribute("aria-disabled", "true");

    await userEvent.click(inBar("Undo"));
    expect(await screen.findByText("0 labelled windows on this chart")).toBeInTheDocument();
    expect(timelineCallsFor(ANCHOR_A.md5)).toBe(3);
    expect(timelineSlider()).not.toHaveAttribute("aria-disabled", "true");
  });

  it("does not move a saved window from the timeline", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await saveNoPattern();
    await roundLoaded("Beta Song");
    await clickTool("Previous");
    await roundLoaded();
    (await timelineShown()).focus();
    await userEvent.keyboard("{ArrowRight}");
    await settle();
    expect(argsOf(calls, "label_move_window")).toEqual([]);
  });

  it("shows the chart's background and star rating in the header, fetched by md5", async () => {
    const calls = renderScreen([WINDOW_A, WINDOW_B], {
      chartBackground: () => ({ mime: "image/png", base64: "iVBORw0KGgo=", width: 1920, height: 1080 }),
    });
    await roundLoaded();
    expect(screen.getByText("mapped by Mapper")).toBeInTheDocument();
    expect(screen.getByTestId("star-rating")).toHaveTextContent("4.50");
    await waitFor(() => {
      expect(document.querySelector("header img")).toHaveAttribute("src", "data:image/png;base64,iVBORw0KGgo=");
    });
    expect(argsOf(calls, "chart_background")).toEqual([{ md5: ANCHOR_A.md5 }]);
  });

  it("falls back to the gradient header when the background cannot be read", async () => {
    renderScreen([WINDOW_A, WINDOW_B], { chartBackground: () => mockIpcError("INTERNAL") });
    await roundLoaded();
    await settle();
    expect(screen.getByTestId("header-fallback")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("draws the window and the examples with the preferred hand layout, again when it changes", async () => {
    const calls = renderScreen([WINDOW_A, WINDOW_B], { settingsGetHandLayout: () => LEFT_THUMB });
    await roundLoaded();
    await waitFor(() => {
      expect(argsOf(calls, "chart_window")).toEqual([{ md5: ANCHOR_A.md5, fromMs: 1000, toMs: 5000, layoutId: LEFT_THUMB }]);
    });
    expect(argsOf(calls, "settings_get_hand_layout")).toEqual([{ keymode: 7 }]);
    expect(argsOf(calls, "label_pattern_examples")).toHaveLength(1);

    act(() => {
      queryClient?.setQueryData(["settings", "handLayout", 7], RIGHT_THUMB);
    });

    await waitFor(() => {
      expect(argsOf(calls, "chart_window").at(-1)).toEqual({ md5: ANCHOR_A.md5, fromMs: 1000, toMs: 5000, layoutId: RIGHT_THUMB });
    });
    await waitFor(() => {
      expect(argsOf(calls, "label_pattern_examples")).toHaveLength(2);
    });
  });

  it("falls back to the profile's default layout when the preference cannot be read", async () => {
    const calls = renderScreen([WINDOW_A, WINDOW_B], { settingsGetHandLayout: () => mockIpcError("INTERNAL") });
    await roundLoaded();
    await waitFor(() => {
      expect(argsOf(calls, "chart_window")).toEqual([{ md5: ANCHOR_A.md5, fromMs: 1000, toMs: 5000, layoutId: null }]);
    });
    expect(await screen.findByTestId("playfield")).toContainHTML("<canvas");
  });

  it("words a failed save and keeps the window", async () => {
    renderScreen([WINDOW_A], {
      labelSubmit: () => mockIpcError("INVALID_INPUT", { pattern: "zz" }, { messageKey: "label.error.unknown_pattern" }),
    });
    await roundLoaded();
    await saveNoPattern();
    expect(await screen.findByRole("alert")).toHaveTextContent("Unknown pattern “zz”.");
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();
    expect(pressed("No pattern")).toBe("true");
  });

  it("shows the done state when no window is left", async () => {
    renderScreen([null]);
    expect(await screen.findByText("No window left to label.")).toBeInTheDocument();
    expect(screen.queryByTestId("playfield")).not.toBeInTheDocument();
  });

  it("keeps the navigation, history and undo usable once the plan is finished", async () => {
    const calls = renderScreen([WINDOW_A], { labelRandom: () => WINDOW_C });
    await roundLoaded();
    await saveNoPattern();

    expect(await screen.findByText("No window left to label.")).toBeInTheDocument();
    expect(
      screen.getByText("Previous still reaches this session's windows; Random and Now playing can open more."),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("playfield")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New session" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Patterns" })).toBeInTheDocument();
    expect(tool("Next")).toHaveAccessibleDescription("The plan has no window left; Random and Now playing still open one.");
    expect(inBar("Skip")).toBeDisabled();
    for (const name of ["Previous", "Random", "Now playing"]) {
      expect(tool(name), name).toHaveAttribute("aria-disabled", "false");
    }

    await clickTool("Previous");
    await roundLoaded("Alpha Song");
    await userEvent.click(inBar("Undo"));
    expect(await screen.findByText("Last label undone.")).toBeInTheDocument();
    expect(argsOf(calls, "label_undo")).toEqual([{ eventId: "01EVENT1" }]);

    await clickTool("Random");
    await roundLoaded("Gamma Song");
    expect(sampleRequests(calls)).toHaveLength(2);
  });

  it("stops and releases the section audio when the plan runs out", async () => {
    renderScreen([WINDOW_A]);
    await roundLoaded();
    const source = await playingSource();

    await saveNoPattern();

    expect(await screen.findByText("No window left to label.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New session" })).toBeInTheDocument();
    expect(source.stops).toBe(1);
    expect(FakeAudioContext.closed).toBe(1);
  });

  it("stops the section audio while the next window is sampled", async () => {
    const calls = renderScreen([WINDOW_A], {
      labelSample: (args) => ((args["req"] as SampleRequestDto).round === 0 ? WINDOW_A : never<LabelWindowDto | null>()),
    });
    await roundLoaded();
    const source = await playingSource();

    await saveNoPattern();

    expect(await screen.findByText("Picking a window…")).toBeInTheDocument();
    expect(sampleRequests(calls)).toHaveLength(2);
    expect(source.stops).toBe(1);
  });

  it("plays silently with a notice when the chart has no audio", async () => {
    renderScreen([WINDOW_A], {
      chartAudio: () => mockIpcError("NOT_FOUND", {}, { messageKey: "error.chart_audio_unavailable" }),
    });
    await roundLoaded();

    expect(await screen.findByText("No audio file was found for this chart in its song folder.")).toBeInTheDocument();
    expect(screen.getByTestId("playfield")).toContainHTML("<canvas");
    await userEvent.click(await screen.findByRole("button", { name: "Play" }));
    expect(await screen.findByRole("button", { name: "Pause" })).toBeInTheDocument();
  });

  it("creates the audio context on the first play only", async () => {
    renderScreen();
    await roundLoaded();
    expect(FakeAudioContext.created).toBe(0);
    await userEvent.click(await screen.findByRole("button", { name: "Play" }));
    expect(await screen.findByRole("button", { name: "Pause" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Pause" }));
    await userEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(FakeAudioContext.created).toBe(1);
  });
});

describe("LabelScreen pattern panel", () => {
  function separator(): HTMLElement {
    return screen.getByRole("separator", { name: "Resize the pattern panel" });
  }

  it("resizes from the keyboard within its bounds and remembers the width", async () => {
    renderScreen();
    await roundLoaded();
    const handle = separator();
    expect(handle).toHaveAttribute("aria-orientation", "vertical");
    expect(handle).toHaveAttribute("aria-valuenow", "416");
    expect(handle).toHaveAttribute("aria-valuemin", "320");
    expect(handle).toHaveAttribute("aria-valuemax", String(Math.floor(window.innerWidth * 0.7)));

    handle.focus();
    await userEvent.keyboard("{ArrowLeft}{ArrowLeft}{ArrowRight}");
    expect(handle).toHaveAttribute("aria-valuenow", "432");
    expect(localStorage.getItem(LABEL_PREFS.panelWidthKey)).toBe("432");
    await userEvent.keyboard("{Home}");
    expect(handle).toHaveAttribute("aria-valuenow", "320");
    await userEvent.keyboard("{End}");
    expect(handle).toHaveAttribute("aria-valuenow", String(Math.floor(window.innerWidth * 0.7)));
    expect(screen.getByTestId("pattern-panel").style.width).toBe(`${Math.floor(window.innerWidth * 0.7)}px`);
  });

  it("resizes by dragging the separator", async () => {
    renderScreen();
    await roundLoaded();
    const handle = separator();
    fireEvent.pointerDown(handle, { clientX: 600, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 500, pointerId: 1 });
    fireEvent.pointerUp(window, { clientX: 500, pointerId: 1 });
    expect(handle).toHaveAttribute("aria-valuenow", "516");
    expect(localStorage.getItem(LABEL_PREFS.panelWidthKey)).toBe("516");
    fireEvent.pointerMove(window, { clientX: 100, pointerId: 1 });
    expect(handle).toHaveAttribute("aria-valuenow", "516");
  });

  it("restores a stored width, clamped to the screen", async () => {
    localStorage.setItem(LABEL_PREFS.panelWidthKey, "5000");
    renderScreen();
    await roundLoaded();
    expect(separator()).toHaveAttribute("aria-valuenow", String(Math.floor(window.innerWidth * 0.7)));
  });
});

describe("LabelScreen player layout", () => {
  function frame(): HTMLElement {
    return screen.getByTestId("player-frame");
  }

  function controls(): HTMLElement {
    return screen.getByRole("group", { name: "Playback controls" });
  }

  it("gives the left column to the playfield, with play, time and timeline over its bottom edge and no toolbar or footer", async () => {
    renderScreen();
    await roundLoaded();
    await timelineShown();
    const column = screen.getByRole("region", { name: "Label" });
    expect(within(column).getByTestId("player-frame")).toContainElement(screen.getByTestId("playfield"));
    expect(frame()).toContainElement(controls());
    expect(within(controls()).getByRole("button", { name: "Play" })).toHaveAttribute("aria-keyshortcuts", "Space");
    expect(within(controls()).getByText("00:01.000–00:05.000")).toBeInTheDocument();
    expect(within(controls()).getByRole("slider", { name: "Window position" })).toBeInTheDocument();
    expect(within(controls()).getByTestId("timeline-track")).toBeInTheDocument();
    expect(within(column).queryByRole("toolbar")).toBeNull();
    expect(screen.queryByRole("contentinfo")).toBeNull();
  });

  it("fades the controls out when idle and brings them back on pointer movement or keyboard focus", async () => {
    renderScreen(undefined, {}, { params: { ...TEST_PARAMS, controlsIdleMs: 30, controlsRevealMs: 30 } });
    await roundLoaded();
    await timelineShown();
    await waitFor(() => {
      expect(controls()).toHaveAttribute("data-visible", "false");
    });
    fireEvent.pointerMove(screen.getByTestId("playfield"));
    expect(controls()).toHaveAttribute("data-visible", "false");
    fireEvent.pointerMove(screen.getByTestId("controls-hover-zone"));
    expect(controls()).toHaveAttribute("data-visible", "true");
    await waitFor(() => {
      expect(controls()).toHaveAttribute("data-visible", "false");
    });
    act(() => {
      timelineSlider().focus();
    });
    expect(controls()).toHaveAttribute("data-visible", "true");
    await wait(90);
    expect(controls()).toHaveAttribute("data-visible", "true");
  });

  it("holds every playback setting in the side flyout, which Escape closes without clearing the answer", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    expect(await screen.findByRole("button", { name: "Playback settings" })).toHaveAttribute("aria-expanded", "false");

    const panel = await openSettings();
    expect(screen.getByRole("button", { name: "Playback settings" })).toHaveAttribute("aria-expanded", "true");
    for (const [role, name] of [
      ["slider", "Audio offset"],
      ["combobox", "Scroll mode"],
      ["spinbutton", "osu! speed"],
      ["slider", "Zoom"],
      ["slider", "Playback rate"],
      ["combobox", "Skin"],
      ["button", "Reload skin"],
      ["checkbox", "Disable percy"],
      ["checkbox", "Show judgements"],
      ["checkbox", "Show combo"],
      ["checkbox", "Key press effect"],
      ["checkbox", "Lighting effect"],
    ] as const) {
      expect(within(panel).getByRole(role, { name }), name).toBeInTheDocument();
    }
    expect(within(panel).getByText("Seed 42")).toBeInTheDocument();
    expect(within(panel).queryByRole("checkbox", { name: "Fit window" })).toBeNull();

    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Playback settings" })).toBeNull();
    expect(screen.getByRole("button", { name: "Playback settings" })).toHaveFocus();
    expect(chips()).toEqual(["regular.stream.jumpstream"]);
  });

  it("shows a large play button over the paused section, which starts it and leaves while it plays", async () => {
    renderScreen();
    await roundLoaded();
    await timelineShown();
    const centre = screen.getByRole("button", { name: "Play the section" });
    expect(screen.getByTestId("player-centre")).toContainElement(centre);

    await userEvent.click(centre);

    expect(await within(controls()).findByRole("button", { name: "Pause" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Play the section" })).toBeNull();
    await waitFor(() => {
      expect(FakeAudioContext.sources.map((s) => s.started)).toEqual([1, 1]);
    });

    await userEvent.click(within(controls()).getByRole("button", { name: "Pause" }));
    expect(await screen.findByRole("button", { name: "Play the section" })).toBeInTheDocument();
  });

  it("starts the section from the large play button with the keyboard", async () => {
    renderScreen();
    await roundLoaded();
    await timelineShown();
    screen.getByRole("button", { name: "Play the section" }).focus();
    await userEvent.keyboard("{Enter}");
    expect(await within(controls()).findByRole("button", { name: "Pause" })).toBeInTheDocument();
  });

  it("puts the time, then back, play and forward at the right end of the controls row", async () => {
    renderScreen();
    await roundLoaded();
    await timelineShown();
    const play = within(controls()).getByRole("button", { name: "Play" });
    const row = [...(play.parentElement?.children ?? [])];
    expect(row.slice(-3)).toEqual([
      within(controls()).getByRole("button", { name: "Back 2 s" }),
      play,
      within(controls()).getByRole("button", { name: "Forward 2 s" }),
    ]);
    expect(within(controls()).getByTestId("playback-readout")).toHaveClass("ml-auto");
  });

  it("plays and pauses on a click on the preview", async () => {
    renderScreen();
    await roundLoaded();
    await timelineShown();
    await userEvent.click(screen.getByTestId("playfield"));
    expect(await within(controls()).findByRole("button", { name: "Pause" })).toBeInTheDocument();
    await userEvent.click(screen.getByTestId("playfield"));
    expect(await screen.findByRole("button", { name: "Play the section" })).toBeInTheDocument();
  });

  it("draws the preview where the playhead stands after a pause and after seeks made while paused or before playing", async () => {
    renderScreen();
    await roundLoaded();
    await timelineShown();
    const previewMs = (): number | null | undefined => vi.mocked(Playfield).mock.lastCall?.[0].position?.();
    const playhead = within(controls()).getByRole("slider", { name: "Playback position" });
    expect(previewMs()).toBeNull();

    await userEvent.click(within(controls()).getByRole("button", { name: "Forward 2 s" }));
    await waitFor(() => {
      expect(playhead).toHaveAttribute("aria-valuenow", "2000");
    });
    expect(previewMs()).toBe(2000);

    await userEvent.click(screen.getByTestId("playfield"));
    expect(await within(controls()).findByRole("button", { name: "Pause" })).toBeInTheDocument();
    await userEvent.click(screen.getByTestId("playfield"));
    await screen.findByRole("button", { name: "Play the section" });
    await userEvent.click(within(controls()).getByRole("button", { name: "Back 2 s" }));
    await waitFor(() => {
      expect(playhead).toHaveAttribute("aria-valuenow", String(previewMs()));
    });
    expect(playhead).toHaveAttribute("aria-valuenow", "0");
  });

  it("keeps a right gutter for the job tray's edge tab, so the tab covers no part of the pattern panel", async () => {
    renderScreen();
    await roundLoaded();
    const root = screen.getByTestId("pattern-panel").parentElement;
    expect(root).toHaveClass("pr-[calc(var(--job-tray-tab-w)+0.25rem)]");
    expect(root).not.toHaveClass("p-4");
  });

  it("seeks with the arrow keys from anywhere on the page, as a video player does, leaving sliders their own arrows", async () => {
    renderScreen();
    await roundLoaded();
    await timelineShown();
    const playhead = within(controls()).getByRole("slider", { name: "Playback position" });
    fireEvent.keyDown(document.body, { key: "ArrowRight" });
    fireEvent.keyDown(document.body, { key: "ArrowRight", repeat: true });
    await waitFor(() => {
      expect(playhead).toHaveAttribute("aria-valuenow", "4000");
    });
    fireEvent.keyDown(document.body, { key: "ArrowLeft" });
    await waitFor(() => {
      expect(playhead).toHaveAttribute("aria-valuenow", "2000");
    });

    fireEvent.keyDown(playhead, { key: "ArrowRight" });
    fireEvent.keyDown(screen.getByRole("button", { name: "Skip" }), { key: "ArrowRight" });
    await settle();
    expect(playhead).toHaveAttribute("aria-valuenow", "4000");
  });

  it("seeks inside the section with back, forward and the playhead, moving and storing nothing", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await timelineShown();
    const playhead = within(controls()).getByRole("slider", { name: "Playback position" });
    // Window 1–5 s with 1 s of preroll and 250 ms of postroll: the section loops 0–5.25 s.
    expect(playhead).toHaveAttribute("aria-valuemin", "0");
    expect(playhead).toHaveAttribute("aria-valuemax", "5250");

    await userEvent.click(within(controls()).getByRole("button", { name: "Forward 2 s" }));
    await waitFor(() => {
      expect(playhead).toHaveAttribute("aria-valuenow", "2000");
    });
    await userEvent.click(within(controls()).getByRole("button", { name: "Back 2 s" }));
    await userEvent.click(within(controls()).getByRole("button", { name: "Back 2 s" }));
    await waitFor(() => {
      expect(playhead).toHaveAttribute("aria-valuenow", "3250");
    });
    playhead.focus();
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => {
      expect(playhead).toHaveAttribute("aria-valuetext", "00:01.250");
    });

    await userEvent.click(within(controls()).getByRole("button", { name: "Play" }));
    await waitFor(() => {
      expect(FakeAudioContext.sources[0]?.offsets).toEqual([1.25]);
    });
    await settle();
    for (const cmd of ["label_move_window", "label_resize_window", "label_submit"]) {
      expect(argsOf(calls, cmd), cmd).toEqual([]);
    }
    expect(screen.getByRole("slider", { name: "Window position" })).toHaveAttribute("aria-valuenow", "1000");
  });

  it("nudges a first-time viewer towards the settings tab until the flyout is opened, then never again", async () => {
    renderScreen();
    await roundLoaded();
    const tab = await screen.findByRole("button", { name: "Playback settings" });
    expect(tab).toHaveAttribute("data-hint", "true");
    expect(tab).toHaveAccessibleDescription("Playback settings live here: offset, speed, skin and more");

    await openSettings();
    expect(localStorage.getItem(LABEL_PREFS.settingsHintSeenKey)).toBe("true");
    await userEvent.keyboard("{Escape}");
    expect(tab).not.toHaveAttribute("data-hint");

    cleanup();
    renderScreen();
    await roundLoaded();
    expect(await screen.findByRole("button", { name: "Playback settings" })).not.toHaveAttribute("data-hint");
  });

  it("drops the nudge once a playback setting changes from the keyboard", async () => {
    renderScreen();
    await roundLoaded();
    expect(await screen.findByRole("button", { name: "Playback settings" })).toHaveAttribute("data-hint", "true");
    await userEvent.keyboard("{F4}");
    expect(screen.getByRole("button", { name: "Playback settings" })).not.toHaveAttribute("data-hint");
    expect(localStorage.getItem(LABEL_PREFS.settingsHintSeenKey)).toBe("true");
  });

  it("draws percy and no other effect by default, and passes each toggle to the playfield, remembered", async () => {
    renderScreen();
    await roundLoaded();
    await timelineShown();
    expect(lastPlayfieldEffects()).toEqual({ percy: true, judgements: false, combo: false, keyPress: false, lighting: false });

    await userEvent.click(await setting("checkbox", "Disable percy"));
    await userEvent.click(await setting("checkbox", "Show judgements"));
    await userEvent.click(await setting("checkbox", "Lighting effect"));

    expect(lastPlayfieldEffects()).toEqual({ percy: false, judgements: true, combo: false, keyPress: false, lighting: true });
    expect(localStorage.getItem(LABEL_PREFS.effectKeys.percy)).toBe("false");
    expect(localStorage.getItem(LABEL_PREFS.effectKeys.judgements)).toBe("true");

    cleanup();
    renderScreen();
    await roundLoaded();
    await timelineShown();
    expect(lastPlayfieldEffects()).toEqual({ percy: false, judgements: true, combo: false, keyPress: false, lighting: true });
    expect(await setting("checkbox", "Disable percy")).toBeChecked();
  });

  it("disables the effects the loaded skin cannot draw, saying why", async () => {
    vi.mocked(skinEffectSupport).mockReturnValue({ ...ALL_EFFECTS_SUPPORTED, percy: false, keyPress: false });
    renderScreen();
    await roundLoaded();
    const percy = await setting("checkbox", "Disable percy");
    expect(percy).toBeDisabled();
    expect(percy).toHaveAccessibleDescription("Needs a skin whose long-note body is percy-style.");
    expect(await setting("checkbox", "Key press effect")).toHaveAccessibleDescription("Needs a skin with pressed-key images.");
    expect(await setting("checkbox", "Show combo")).toBeEnabled();
    expect(vi.mocked(skinEffectSupport)).toHaveBeenLastCalledWith(null);
  });

  it("plays the section at the chosen playback rate and remembers it", async () => {
    renderScreen();
    await roundLoaded();
    const rate = await setting("slider", "Playback rate");
    expect(rate).toHaveValue("1");
    expect(rate).toHaveAttribute("min", "0.5");
    expect(rate).toHaveAttribute("max", "1.5");
    expect(rate).toHaveAttribute("step", "0.05");
    fireEvent.change(rate, { target: { value: "1.25" } });
    expect(localStorage.getItem(LABEL_PREFS.playbackRateKey)).toBe("1.25");
    expect(within(controls()).getByText("×1.25")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Play" }));
    await waitFor(() => {
      expect(FakeAudioContext.sources.length).toBeGreaterThan(0);
    });
    expect(FakeAudioContext.sources.map((s) => s.playbackRate.value)).toEqual([1.25, 1.25]);
  });

  it("applies a dragged playback rate once, on release, instead of restarting the loop on every step", async () => {
    renderScreen();
    await roundLoaded();
    await playingSource();
    const rate = await setting("slider", "Playback rate");
    const before = FakeAudioContext.sources.length;

    fireEvent.pointerDown(rate, { button: 0, pointerId: 1 });
    for (const value of ["1.05", "1.1", "1.15", "1.2"]) {
      fireEvent.change(rate, { target: { value } });
    }
    expect(within(await openSettings()).getByText("×1.20")).toBeInTheDocument();
    expect(FakeAudioContext.sources).toHaveLength(before);
    expect(localStorage.getItem(LABEL_PREFS.playbackRateKey)).toBeNull();

    fireEvent.pointerUp(window, { pointerId: 1 });

    await waitFor(() => {
      expect(FakeAudioContext.sources.at(-1)?.playbackRate.value).toBe(1.2);
    });
    expect(FakeAudioContext.sources).toHaveLength(before + 2);
    expect(localStorage.getItem(LABEL_PREFS.playbackRateKey)).toBe("1.2");
  });

  it("starts at a stored playback rate", async () => {
    localStorage.setItem(LABEL_PREFS.playbackRateKey, "0.75");
    renderScreen();
    await roundLoaded();
    expect(await setting("slider", "Playback rate")).toHaveValue("0.75");
  });

  it("resizes the window from the timeline's edge handles, reloading the chart without a new sample", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await timelineShown();
    screen.getByRole("slider", { name: "Window end" }).focus();
    await userEvent.keyboard("{PageUp}");

    expect(await screen.findByText("00:01.000–00:10.000")).toBeInTheDocument();
    expect(argsOf(calls, "label_resize_window")).toEqual([{ req: { anchor: ANCHOR_A, t0Ms: 1000, t1Ms: 10_000 } }]);
    await waitFor(() => {
      expect(argsOf(calls, "chart_window").at(-1)).toMatchObject({ fromMs: 1000, toMs: 10_000 });
    });

    screen.getByRole("slider", { name: "Window start" }).focus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(await screen.findByText("00:00.000–00:10.000")).toBeInTheDocument();
    expect(argsOf(calls, "label_resize_window").at(-1)).toEqual({
      req: { anchor: { ...ANCHOR_A, t1Ms: 10_000 }, t0Ms: 0, t1Ms: 10_000 },
    });
    expect(argsOf(calls, "label_move_window")).toEqual([]);
    expect(sampleRequests(calls)).toHaveLength(1);
  });

  it("keeps the window and words the error when a resize fails", async () => {
    renderScreen([WINDOW_A, WINDOW_B], { labelResizeWindow: () => mockIpcError("NOT_FOUND") });
    await roundLoaded();
    await timelineShown();
    screen.getByRole("slider", { name: "Window end" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(await within(answerBar()).findByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("00:01.000–00:05.000")).toBeInTheDocument();
  });

  it("puts Previous, Next, Random and Now playing in the map card, with the session counters", async () => {
    renderScreen();
    await roundLoaded();
    const header = screen.getByRole("heading", { name: "Alpha Song" }).closest("header");
    if (header === null) {
      throw new Error("the title is not in the map card");
    }
    const nav = within(header).getByRole("toolbar", { name: "Windows" });
    expect(within(nav).getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual([
      "Previous",
      "Next",
      "Random",
      "Now playing",
    ]);
    expect(within(header).getByText("Gold set: 37")).toBeInTheDocument();
    expect(within(header).getByText("Labelled: 0")).toBeInTheDocument();
  });

  it("shows the map's full details from chart_details in the header dialog", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await waitFor(() => {
      expect(argsOf(calls, "chart_details")).toEqual([{ md5: ANCHOR_A.md5 }]);
    });
    await userEvent.click(screen.getByRole("button", { name: "Show the full image and map details" }));
    const dialog = screen.getByRole("dialog", { name: "Alpha Song" });
    expect(within(dialog).getByRole("list", { name: "Map statistics" })).toHaveTextContent("02:34Length");
    expect(within(dialog).getByText("Source", { selector: "dt" }).nextElementSibling).toHaveTextContent("Some Game");
  });

  it("takes a clicked chip to its pattern card, opening the card's axis but leaving focus on the page", async () => {
    const scrollIntoView = vi.fn();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { value: scrollIntoView, configurable: true, writable: true });
    try {
      renderScreen();
      await roundLoaded();
      await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
      await userEvent.click(screen.getByRole("button", { name: "Stream" }));
      expect(screen.queryByRole("button", { name: "js jumpstream" })).toBeNull();

      await userEvent.click(inBar("Show jumpstream in the pattern panel"));

      const card = await screen.findByRole("button", { name: "js jumpstream" });
      expect(card).toHaveAttribute("data-highlighted", "true");
      expect(scrollIntoView.mock.contexts).toContain(card);
      expect(document.body).toHaveFocus();
    } finally {
      Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
    }
  });

  it("moves focus to the pattern card when a chip is activated from the keyboard", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    inBar("Show jumpstream in the pattern panel").focus();

    await userEvent.keyboard("{Enter}");

    expect(await screen.findByRole("button", { name: "js jumpstream" })).toHaveFocus();
    expect(chips()).toEqual(["regular.stream.jumpstream"]);
  });
});

describe("LabelScreen scroll and zoom controls", () => {
  function osuSpeed(): HTMLElement {
    return within(screen.getByRole("dialog", { name: "Playback settings" })).getByRole("spinbutton", { name: "osu! speed" });
  }

  it("starts in the osu! mode at the default speed, overridable by a prop", async () => {
    renderScreen(undefined, {}, { defaultOsuSpeed: 30 });
    await roundLoaded();
    expect(await setting("combobox", "Scroll mode")).toHaveValue("osu");
    expect(osuSpeed()).toHaveValue(30);
    expect(screen.queryByRole("slider", { name: "Scroll speed" })).not.toBeInTheDocument();
  });

  it("defaults the osu! speed to 20 without a prop", async () => {
    renderScreen();
    await roundLoaded();
    expect(await setting("spinbutton", "osu! speed")).toHaveValue(20);
  });

  it("changes the osu! speed with F3 and F4 even while typing in the pattern search, and keeps it", async () => {
    renderScreen();
    await roundLoaded();
    await timelineShown();
    const search = screen.getByRole("searchbox", { name: "Search patterns" });
    await userEvent.type(search, "js");
    await userEvent.keyboard("{F4}{F4}{F3}{F4}");
    expect(search).toHaveValue("js");
    expect(search).toHaveFocus();
    expect(localStorage.getItem(LABEL_PREFS.osuSpeedKey)).toBe("22");
    await openSettings();
    expect(osuSpeed()).toHaveValue(22);

    await userEvent.keyboard("{F3}");
    expect(osuSpeed()).toHaveValue(21);
  });

  it("stops F3 and F4 at the ends of the 1..40 range", async () => {
    renderScreen(undefined, {}, { defaultOsuSpeed: 40 });
    await roundLoaded();
    await setting("spinbutton", "osu! speed");
    await userEvent.keyboard("{F4}");
    expect(osuSpeed()).toHaveValue(40);
  });

  it("switches to the px/ms slider, carrying over the old px/ms preference", async () => {
    localStorage.setItem(LABEL_PREFS.legacyScrollKey, "1.25");
    renderScreen();
    await roundLoaded();
    await userEvent.selectOptions(await setting("combobox", "Scroll mode"), "pxPerMs");
    expect(screen.getByRole("slider", { name: "Scroll speed" })).toHaveValue("1.25");
    expect(screen.queryByRole("spinbutton", { name: "osu! speed" })).not.toBeInTheDocument();
    expect(localStorage.getItem(LABEL_PREFS.scrollKindKey)).toBe("pxPerMs");
  });

  it("keeps the zoom and draws the playfield without a fixed maximum width", async () => {
    renderScreen();
    await roundLoaded();
    const zoom = await setting("slider", "Zoom");
    fireEvent.change(zoom, { target: { value: "1.5" } });
    expect(zoom).toHaveValue("1.5");
    expect(localStorage.getItem(LABEL_PREFS.zoomKey)).toBe("1.5");
    expect(screen.getByTestId("playfield").querySelector("[class*='max-w']")).toBeNull();
  });
});

describe("LabelScreen skins", () => {
  interface RecordingCanvas {
    images: unknown[];
    fills: number;
  }
  let canvas: RecordingCanvas;
  let bitmaps: { close: ReturnType<typeof vi.fn> }[];

  class SizedResizeObserver {
    constructor(private readonly callback: ResizeObserverCallback) {}
    observe(target: Element): void {
      const entry = {
        target,
        contentRect: { width: 800, height: 600 },
        borderBoxSize: [{ inlineSize: 800, blockSize: 600 }],
      };
      queueMicrotask(() => {
        this.callback([entry as unknown as ResizeObserverEntry], this);
      });
    }
    unobserve(): void {
      // Sized once on observe; nothing to stop.
    }
    disconnect(): void {
      // See unobserve.
    }
  }

  beforeEach(() => {
    canvas = { images: [], fills: 0 };
    bitmaps = [];
    vi.stubGlobal("ResizeObserver", SizedResizeObserver);
    vi.stubGlobal("requestAnimationFrame", () => 0);
    vi.stubGlobal("cancelAnimationFrame", () => undefined);
    vi.stubGlobal(
      "createImageBitmap",
      vi.fn(async (blob: Blob) => {
        if (blob.type === BROKEN_MIME) {
          return Promise.reject(new DOMException("undecodable", "InvalidStateError"));
        }
        const bitmap = { width: 100, height: 50, close: vi.fn() };
        bitmaps.push(bitmap);
        return Promise.resolve(bitmap);
      }),
    );
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
      fillStyle: "#000",
      globalAlpha: 1,
      fillRect: () => {
        canvas.fills++;
      },
      drawImage: (image: unknown) => {
        canvas.images.push(image);
      },
      setTransform: () => undefined,
      save: () => undefined,
      restore: () => undefined,
      translate: () => undefined,
      scale: () => undefined,
    } as unknown as RenderingContext);
  });

  const LIST: SkinListDto = {
    skins: [skinEntry("Alpha4K", [4]), skinEntry("Pilot", [4, 7]), skinEntry("Zeta", [7])],
    current: "Pilot",
    maniaSpeed: null,
    maniaSpeedBpmScale: false,
  };

  function picker(): HTMLSelectElement {
    return within(screen.getByRole("dialog", { name: "Playback settings" })).getByRole("combobox", { name: "Skin" });
  }

  async function pickerReady(): Promise<HTMLSelectElement> {
    await openSettings();
    await waitFor(() => {
      expect(picker()).toBeEnabled();
    });
    return picker();
  }

  it("lists the skins with a block for the keymode first, the rest marked defaults, and starts on the cfg skin", async () => {
    const calls = renderScreen(undefined, { skinList: () => LIST });
    await roundLoaded();
    const select = await pickerReady();
    expect([...select.options].map((o) => o.text)).toEqual([
      "None (procedural)",
      "Pilot",
      "Zeta",
      "Alpha4K (defaults)",
    ]);
    expect(select).toHaveValue("Pilot");
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toEqual([{ folder: "Pilot", keymode: 7 }]);
    });
    await waitFor(() => {
      expect(canvas.images).toContain(bitmaps[0]);
    });
  });

  it("shows the skin effects as loading, not as unsupported, until the chosen skin arrives", async () => {
    vi.mocked(skinEffectSupport).mockReturnValue({ ...ALL_EFFECTS_SUPPORTED, percy: false, keyPress: false, lighting: false });
    let answerSkin: (dto: SkinDto) => void = () => undefined;
    renderScreen(undefined, {
      skinList: () => LIST,
      skinGet: (args) =>
        new Promise<SkinDto>((resolve) => {
          answerSkin = () => {
            resolve(skinDto(String(args["folder"]), Number(args["keymode"])));
          };
        }),
    });
    await roundLoaded();
    await pickerReady();
    const settings = screen.getByRole("dialog", { name: "Playback settings" });
    expect(await within(settings).findByText("Loading the skin…")).toBeInTheDocument();
    expect(within(settings).getByRole("checkbox", { name: "Key press effect" })).toBeDisabled();
    expect(within(settings).queryByText(/Needs a skin/)).toBeNull();

    act(() => {
      answerSkin(skinDto("Pilot", 7));
    });
    await waitFor(() => {
      expect(canvas.images).toContain(bitmaps[0]);
    });
    expect(within(settings).queryByText("Loading the skin…")).toBeNull();
    expect(within(settings).getByText("Needs a skin with pressed-key images.")).toBeInTheDocument();
  });

  it("starts on None when the cfg names no listed skin, and draws procedurally", async () => {
    const calls = renderScreen(undefined, { skinList: () => ({ ...LIST, current: null }) });
    await roundLoaded();
    expect(await pickerReady()).toHaveValue("");
    await waitFor(() => {
      expect(canvas.fills).toBeGreaterThan(0);
    });
    expect(argsOf(calls, "skin_get")).toEqual([]);
    expect(canvas.images).toEqual([]);
  });

  it("lets a stored choice win over the cfg skin", async () => {
    localStorage.setItem(LABEL_PREFS.skinKey, JSON.stringify({ folder: "Zeta" }));
    const calls = renderScreen(undefined, { skinList: () => LIST });
    await roundLoaded();
    expect(await pickerReady()).toHaveValue("Zeta");
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toEqual([{ folder: "Zeta", keymode: 7 }]);
    });
  });

  it("keeps a stored None over the cfg skin", async () => {
    localStorage.setItem(LABEL_PREFS.skinKey, JSON.stringify({ folder: null }));
    const calls = renderScreen(undefined, { skinList: () => LIST });
    await roundLoaded();
    expect(await pickerReady()).toHaveValue("");
    expect(argsOf(calls, "skin_get")).toEqual([]);
  });

  it("switches to None: stored, the skin's bitmaps closed, and the playfield drawn with no image", async () => {
    renderScreen(undefined, { skinList: () => LIST });
    await roundLoaded();
    await waitFor(() => {
      expect(canvas.images.length).toBeGreaterThan(0);
    });
    await userEvent.selectOptions(await pickerReady(), "");
    expect(localStorage.getItem(LABEL_PREFS.skinKey)).toBe(JSON.stringify({ folder: null }));
    expect(bitmaps[0]?.close).toHaveBeenCalledTimes(1);
    canvas.images.length = 0;
    canvas.fills = 0;
    fireEvent.change(screen.getByRole("slider", { name: "Zoom" }), { target: { value: "1.25" } });
    await waitFor(() => {
      expect(canvas.fills).toBeGreaterThan(0);
    });
    expect(canvas.images).toEqual([]);
  });

  it("lets judgements chosen on the procedural stage be turned off after switching to a skin without hit art", async () => {
    localStorage.setItem(LABEL_PREFS.effectKeys.judgements, "true");
    vi.mocked(skinEffectSupport).mockImplementation((skin) =>
      skin === null ? ALL_EFFECTS_SUPPORTED : { ...ALL_EFFECTS_SUPPORTED, judgements: false },
    );
    renderScreen(undefined, { skinList: () => ({ ...LIST, current: null }) });
    await roundLoaded();
    await userEvent.selectOptions(await pickerReady(), "Zeta");
    await waitFor(() => {
      expect(vi.mocked(skinEffectSupport).mock.lastCall?.[0]).not.toBeNull();
    });
    const judgements = await setting("checkbox", "Show judgements");
    expect(judgements).toBeEnabled();
    expect(judgements).toBeChecked();
    expect(judgements).toHaveAccessibleDescription("This skin has no judgement images: they show as text.");
    await userEvent.click(judgements);
    expect(lastPlayfieldEffects()?.judgements).toBe(false);
  });

  it("closes the skin's bitmaps when the screen unmounts", async () => {
    renderScreen(undefined, { skinList: () => LIST });
    await roundLoaded();
    await waitFor(() => {
      expect(bitmaps).toHaveLength(1);
    });
    cleanup();
    expect(bitmaps[0]?.close).toHaveBeenCalledTimes(1);
  });

  it("draws an image the skin could not decode procedurally and says so", async () => {
    renderScreen(undefined, {
      skinList: () => LIST,
      skinGet: (args) => skinDto(String(args["folder"]), Number(args["keymode"]), BROKEN_MIME),
    });
    await roundLoaded();
    expect(await screen.findByText(/could not be decoded/)).toHaveTextContent("note.3");
    await waitFor(() => {
      expect(canvas.fills).toBeGreaterThan(0);
    });
    expect(canvas.images).toEqual([]);
  });

  it("fetches a skin once per folder, keymode and ini mtime across rounds, and again on Reload", async () => {
    let mtime = "1";
    const calls = renderScreen(undefined, {
      skinList: () => ({ ...LIST, skins: [skinEntry("Pilot", [7], mtime)] }),
      // Slow enough that the list's new mtime renders while Reload's own skin fetch is still in flight.
      skinGet: async (args) => {
        await new Promise((resolve) => setTimeout(resolve, 50));
        return skinDto(String(args["folder"]), Number(args["keymode"]));
      },
    });
    await roundLoaded();
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toHaveLength(1);
    });
    await holdSkip();
    await roundLoaded("Beta Song");
    expect(argsOf(calls, "skin_get")).toHaveLength(1);

    mtime = "2";
    await userEvent.click(await setting("button", "Reload skin"));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Reload skin" })).toBeEnabled();
    });
    await settle();
    expect(argsOf(calls, "skin_get")).toHaveLength(2);
    expect(argsOf(calls, "skin_list")).toHaveLength(2);

    await userEvent.click(screen.getByRole("button", { name: "Reload skin" }));
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toHaveLength(3);
    });
    expect(argsOf(calls, "skin_list")).toHaveLength(3);
  });

  it("fetches the stored skin before the skin list answers, and keeps it when the list confirms it", async () => {
    localStorage.setItem(LABEL_PREFS.skinKey, JSON.stringify({ folder: "Zeta" }));
    let answerList: (list: SkinListDto) => void = () => undefined;
    const pendingList = new Promise<SkinListDto>((resolve) => {
      answerList = resolve;
    });
    const calls = renderScreen(undefined, { skinList: () => pendingList });
    await roundLoaded();
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toEqual([{ folder: "Zeta", keymode: 7 }]);
    });
    expect(argsOf(calls, "skin_list")).toHaveLength(1);
    await waitFor(() => {
      expect(canvas.images).toContain(bitmaps[0]);
    });

    answerList(LIST);
    expect(await pickerReady()).toHaveValue("Zeta");
    await settle();
    expect(argsOf(calls, "skin_get")).toHaveLength(1);
    expect(bitmaps[0]?.close).not.toHaveBeenCalled();
  });

  it("falls back silently to the cfg skin when a stored skin is gone, with no error before the list answers", async () => {
    localStorage.setItem(LABEL_PREFS.skinKey, JSON.stringify({ folder: "OldSkin" }));
    let answerList: (list: SkinListDto) => void = () => undefined;
    const pendingList = new Promise<SkinListDto>((resolve) => {
      answerList = resolve;
    });
    const missing = "This skin was not found in the osu! Skins folder.";
    const seen: string[] = [];
    const observer = new MutationObserver(() => {
      seen.push(...[...document.querySelectorAll('[role="status"]')].map((el) => el.textContent));
    });
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
    const calls = renderScreen(undefined, {
      skinList: () => pendingList,
      skinGet: (args) =>
        args["folder"] === "OldSkin"
          ? mockIpcError("NOT_FOUND", {}, { messageKey: "error.skin_unavailable" })
          : skinDto(String(args["folder"]), Number(args["keymode"])),
    });
    try {
      await roundLoaded();
      await waitFor(() => {
        expect(argsOf(calls, "skin_get")).toEqual([{ folder: "OldSkin", keymode: 7 }]);
      });
      await settle();
      expect(screen.queryByText(missing)).not.toBeInTheDocument();

      answerList(LIST);
      expect(await pickerReady()).toHaveValue("Pilot");
      await waitFor(() => {
        expect(canvas.images).toContain(bitmaps[0]);
      });
      expect(argsOf(calls, "skin_get")).toEqual([
        { folder: "OldSkin", keymode: 7 },
        { folder: "Pilot", keymode: 7 },
      ]);
      await settle();
      expect(seen).not.toContain(missing);
      expect(screen.queryByText(missing)).not.toBeInTheDocument();
    } finally {
      observer.disconnect();
    }
  });

  it("fetches the cfg skin as soon as the list names it when nothing is stored", async () => {
    const calls = renderScreen(undefined, { skinList: () => LIST });
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toEqual([{ folder: "Pilot", keymode: 7 }]);
    });
    await settle();
    expect(argsOf(calls, "skin_get")).toHaveLength(1);
  });

  it("refetches the skin exactly once when a later list reports a new ini mtime", async () => {
    let mtime = "1";
    const calls = renderScreen(undefined, {
      skinList: () => ({ ...LIST, skins: [skinEntry("Pilot", [7], mtime)] }),
    });
    await roundLoaded();
    await waitFor(() => {
      expect(bitmaps).toHaveLength(1);
    });

    mtime = "2";
    await act(async () => {
      await queryClient?.invalidateQueries({ queryKey: skinKeys.list() });
    });
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toHaveLength(2);
    });
    await settle();
    expect(argsOf(calls, "skin_get")).toHaveLength(2);
    expect(argsOf(calls, "skin_list")).toHaveLength(2);

    await act(async () => {
      await queryClient?.invalidateQueries({ queryKey: skinKeys.list() });
    });
    await settle();
    expect(argsOf(calls, "skin_list")).toHaveLength(3);
    expect(argsOf(calls, "skin_get")).toHaveLength(2);
  });

  it("defaults the osu! speed to the cfg ManiaSpeed until one is stored", async () => {
    renderScreen(undefined, { skinList: () => ({ ...LIST, maniaSpeed: 30 }) });
    await roundLoaded();
    await openSettings();
    await waitFor(() => {
      expect(screen.getByRole("spinbutton", { name: "osu! speed" })).toHaveValue(30);
    });
    await userEvent.keyboard("{F4}");
    expect(screen.getByRole("spinbutton", { name: "osu! speed" })).toHaveValue(31);
    expect(localStorage.getItem(LABEL_PREFS.osuSpeedKey)).toBe("31");
  });

  it("keeps a stored osu! speed over the cfg ManiaSpeed", async () => {
    localStorage.setItem(LABEL_PREFS.osuSpeedKey, "12");
    const calls = renderScreen(undefined, { skinList: () => ({ ...LIST, maniaSpeed: 30 }) });
    await roundLoaded();
    await waitFor(() => {
      expect(argsOf(calls, "skin_list")).toHaveLength(1);
    });
    await pickerReady();
    expect(screen.getByRole("spinbutton", { name: "osu! speed" })).toHaveValue(12);
  });

  it("notes that BPM-scaled speed is not reproduced when the cfg enables it", async () => {
    renderScreen(undefined, { skinList: () => ({ ...LIST, maniaSpeedBpmScale: true }) });
    await roundLoaded();
    expect(await screen.findByText(/BPM/)).toBeInTheDocument();
  });
});
