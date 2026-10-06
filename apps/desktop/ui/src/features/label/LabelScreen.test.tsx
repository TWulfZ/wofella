import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AnchorDto,
  ChartWindowDto,
  LabelStatsDto,
  LabelWindowDto,
  PatternDefDto,
  SampleRequestDto,
} from "@/ipc/bindings";
import { type CommandHandlers, type MockCall, mockCommands, mockIpcError } from "@/ipc/mocks";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { LabelScreen } from "./LabelScreen";

const TAXONOMY: PatternDefDto[] = [
  { id: "regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "exactly two notes in one column" },
  { id: "regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "three or more notes in one column" },
  { id: "regular.stream.jumpstream", axis: "7k.regular.stream", key: "js", description: "stream with two-note chords" },
];

const ANCHOR_A: AnchorDto = { md5: "a".repeat(32), t0Ms: 1000, t1Ms: 5000, cols: [1, 2, 3, 4, 5, 6, 7] };
const ANCHOR_B: AnchorDto = { md5: "b".repeat(32), t0Ms: 20_000, t1Ms: 24_000, cols: [1, 2, 3, 4, 5, 6, 7] };

function labelWindow(anchor: AnchorDto, title: string): LabelWindowDto {
  return { anchor, title, artist: "Artist", version: "Insane", level: "dan:7", stratum: "dan_07/nps_2", played: true };
}

const WINDOW_A = labelWindow(ANCHOR_A, "Alpha Song");
const WINDOW_B = labelWindow(ANCHOR_B, "Beta Song");

function chartWindow(md5: string, fromMs: number, toMs: number): ChartWindowDto {
  return {
    md5,
    keymode: 7,
    fromMs,
    toMs,
    notes: [{ tMs: fromMs, col: 3, endMs: null }],
    timing: [{ tMs: 0, kind: "red", beatLenMs: 300, meter: 4, sv: null }],
    layout: {
      id: "k7.313_right_thumb",
      columns: ["left", "left", "left", "right", "right", "right", "right"].map((hand) => ({
        hand: hand as "left" | "right",
        finger: "index" as const,
      })),
    },
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
};

interface FakeSourceNode {
  started: number;
  stops: number;
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
    const node: FakeSourceNode = { started: 0, stops: 0 };
    FakeAudioContext.sources.push(node);
    return {
      buffer: null,
      loop: false,
      loopStart: 0,
      loopEnd: 0,
      connect: () => undefined,
      disconnect: () => undefined,
      start: () => {
        node.started++;
      },
      stop: () => {
        node.stops++;
      },
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
  FakeAudioContext.created = 0;
  FakeAudioContext.closed = 0;
  FakeAudioContext.sources = [];
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

function renderScreen(windows: (LabelWindowDto | null)[] = [WINDOW_A, WINDOW_B], extra: CommandHandlers = {}) {
  let eventSeq = 0;
  const calls = mockCommands({
    labelTaxonomy: () => TAXONOMY,
    labelStats: () => STATS,
    labelSample: (args) => windows[(args["req"] as SampleRequestDto).round] ?? null,
    chartWindow: (args) => chartWindow(String(args["md5"]), Number(args["fromMs"]), Number(args["toMs"])),
    chartAudio: () => ({ mime: "audio/mpeg", base64: "SUQz" }),
    labelResolvePatterns: (args) =>
      (args["tokens"] as string[]).map((token) => TAXONOMY.find((p) => p.key === token)?.id ?? token),
    labelSubmit: () => {
      eventSeq++;
      return { id: `01EVENT${eventSeq}` };
    },
    labelUndo: () => null,
    labelReshape: (args) => {
      const anchor = args["anchor"] as AnchorDto;
      const half = (anchor.t1Ms - anchor.t0Ms) / 2;
      return { ...anchor, t0Ms: anchor.t0Ms + half, t1Ms: anchor.t1Ms + half };
    },
    ...extra,
  });
  renderWithRouter(<LabelScreen keymode={7} seed="42" />, { path: "/label" });
  return calls;
}

async function roundLoaded(title = "Alpha Song"): Promise<void> {
  expect(await screen.findByRole("heading", { name: title })).toBeInTheDocument();
}

function answerBox(): HTMLElement {
  return screen.getByRole("textbox", { name: "Answer" });
}

function submitted(calls: MockCall[]): unknown[] {
  return argsOf(calls, "label_submit").map((a) => a["req"]);
}

function pressed(name: string): string | null {
  return screen.getByRole("button", { name }).getAttribute("aria-pressed");
}

async function playingSource(): Promise<FakeSourceNode> {
  await userEvent.click(await screen.findByRole("button", { name: "Play" }));
  await waitFor(() => {
    expect(FakeAudioContext.sources.map((s) => s.started)).toEqual([1]);
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
      expect(argsOf(calls, "chart_window")).toEqual([{ md5: ANCHOR_A.md5, fromMs: 1000, toMs: 5000, layoutId: null }]);
    });
    expect(await screen.findByTestId("playfield")).toContainHTML("<canvas");
    expect(screen.getByText("00:01.000–00:05.000")).toBeInTheDocument();
    expect(screen.getByText("Insane")).toBeInTheDocument();
    expect(screen.getByText("Played")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "js jumpstream" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByText("Gold set: 37")).toBeInTheDocument();
  });

  it("submits the clicked chips and flags on Enter", async () => {
    const calls = renderScreen();
    await roundLoaded();

    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    await userEvent.click(screen.getByRole("button", { name: "mj minijack" }));
    await userEvent.click(screen.getByRole("button", { name: "Mixed" }));
    expect(answerBox()).toHaveValue("js mj");
    expect(screen.getByRole("button", { name: "Mixed" })).toHaveAttribute("aria-pressed", "true");
    await userEvent.type(answerBox(), "{Enter}");

    await roundLoaded("Beta Song");
    expect(argsOf(calls, "label_resolve_patterns")).toEqual([{ keymode: 7, tokens: ["js", "mj"] }]);
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
    expect(sampleRequests(calls)[1]).toMatchObject({ round: 1, exclude: [ANCHOR_A] });
    expect(screen.getByText("Labelled: 1")).toBeInTheDocument();
    expect(answerBox()).toHaveValue("");
  });

  it("submits a typed REPL line, reflecting its tokens on the chips", async () => {
    const calls = renderScreen();
    await roundLoaded();

    await userEvent.type(answerBox(), "lj m tl");
    expect(screen.getByRole("button", { name: "lj longjack" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Mixed" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Left thumb" })).toHaveAttribute("aria-pressed", "true");
    await userEvent.type(answerBox(), "{Enter}");

    await roundLoaded("Beta Song");
    expect(submitted(calls)).toEqual([
      {
        anchor: ANCHOR_A,
        patterns: ["regular.jack.longjack"],
        noPattern: false,
        mixed: true,
        unsure: false,
        thumbPref: "left",
      },
    ]);
  });

  it("stores x as no pattern without resolving anything", async () => {
    const calls = renderScreen();
    await roundLoaded();

    await userEvent.type(answerBox(), "x{Enter}");

    await roundLoaded("Beta Song");
    expect(argsOf(calls, "label_resolve_patterns")).toEqual([]);
    expect(submitted(calls)).toEqual([
      { anchor: ANCHOR_A, patterns: [], noPattern: true, mixed: false, unsure: false, thumbPref: null },
    ]);
  });

  it("skips with s: nothing stored, the next sample excludes the shown window", async () => {
    const calls = renderScreen();
    await roundLoaded();

    await userEvent.type(answerBox(), "s{Enter}");

    await roundLoaded("Beta Song");
    expect(submitted(calls)).toEqual([]);
    expect(sampleRequests(calls)[1]).toMatchObject({ seed: "42", round: 1, exclude: [ANCHOR_A] });
    expect(screen.getByText("Skipped: 1")).toBeInTheDocument();
  });

  it("types keys pressed outside the answer box into it instead of running them as commands", async () => {
    const calls = renderScreen();
    await roundLoaded();

    answerBox().blur();
    await userEvent.keyboard("st");
    expect(answerBox()).toHaveValue("st");
    expect(answerBox()).toHaveFocus();
    answerBox().blur();
    await userEvent.keyboard(" bu");
    expect(answerBox()).toHaveValue("st bu");
    expect(screen.getByText("Skipped: 0")).toBeInTheDocument();
    expect(argsOf(calls, "label_undo")).toEqual([]);

    answerBox().blur();
    await userEvent.keyboard("{Enter}");
    await roundLoaded("Beta Song");
    expect(argsOf(calls, "label_resolve_patterns")).toEqual([{ keymode: 7, tokens: ["st", "bu"] }]);
  });

  it("toggles play with Space outside the answer box only while it is empty", async () => {
    renderScreen();
    await roundLoaded();
    answerBox().blur();
    await userEvent.keyboard(" ");
    expect(await screen.findByRole("button", { name: "Pause" })).toBeInTheDocument();
  });

  it("keeps focus in the answer box when Play or the playfield is clicked", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.type(answerBox(), "js");

    await userEvent.click(await screen.findByRole("button", { name: "Play" }));
    expect(answerBox()).toHaveFocus();
    await userEvent.click(screen.getByTestId("playfield"));
    expect(answerBox()).toHaveFocus();
    await userEvent.keyboard(" mj");
    expect(answerBox()).toHaveValue("js mj");
  });

  it("sends keys typed on a slider or the fit checkbox to the answer box, and takes focus back after using them", async () => {
    renderScreen();
    await roundLoaded();
    const offset = await screen.findByRole("slider", { name: "Audio offset" });

    offset.focus();
    await userEvent.keyboard("js");
    expect(answerBox()).toHaveValue("js");
    expect(answerBox()).toHaveFocus();

    await userEvent.pointer([
      { keys: "[MouseLeft>]", target: offset },
      { keys: "[/MouseLeft]", target: offset },
    ]);
    expect(answerBox()).toHaveFocus();
    await userEvent.click(screen.getByRole("checkbox", { name: "Fit window" }));
    expect(answerBox()).toHaveFocus();
  });

  it("toggles play once while Space is held in the empty answer box", async () => {
    renderScreen();
    await roundLoaded();
    await screen.findByRole("button", { name: "Play" });
    answerBox().focus();
    await userEvent.keyboard("[Space>2]");
    expect(await screen.findByRole("button", { name: "Pause" })).toBeInTheDocument();
    expect(answerBox()).toHaveValue("");
  });

  it("does not toggle play on a Space typed between pattern keys", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.type(answerBox(), "js mj");
    expect(answerBox()).toHaveValue("js mj");
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
  });

  it("stores one label while Enter is held across the round change", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.type(answerBox(), "x");
    await userEvent.keyboard("{Enter>20}");
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toHaveLength(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("ignores a second Enter that lands before the next round renders", async () => {
    let form: HTMLFormElement | null = null;
    const calls = renderScreen(undefined, {
      labelSubmit: () => {
        // Many microtask hops let the first submit settle, while React's render waits for a macrotask.
        void (async () => {
          for (let i = 0; i < 1000; i++) {
            await Promise.resolve();
          }
          if (form !== null) {
            fireEvent.submit(form);
          }
        })();
        return { id: "01EVENT1" };
      },
    });
    await roundLoaded();
    await userEvent.type(answerBox(), "x");
    form = answerBox().closest("form");
    fireEvent.submit(answerBox());

    await roundLoaded("Beta Song");
    expect(submitted(calls)).toHaveLength(1);
    expect(screen.getByText("Labelled: 1")).toBeInTheDocument();
  });

  it("clears the draft with Escape or the Clear button", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.type(answerBox(), "js mj");
    await userEvent.keyboard("{Escape}");
    expect(answerBox()).toHaveValue("");
    expect(pressed("js jumpstream")).toBe("false");

    await userEvent.click(screen.getByRole("button", { name: "js jumpstream" }));
    await userEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(answerBox()).toHaveValue("");
    expect(answerBox()).toHaveFocus();
  });

  it("keeps the chip selection when reshape, help or undo come from a button", async () => {
    renderScreen();
    await roundLoaded();
    await userEvent.click(await screen.findByRole("button", { name: "js jumpstream" }));

    await userEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(await screen.findByText("00:03.000–00:07.000")).toBeInTheDocument();
    expect(answerBox()).toHaveValue("js");
    await userEvent.click(screen.getByRole("button", { name: "Help" }));
    expect(answerBox()).toHaveValue("js");
    await userEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(await screen.findByText("Nothing to undo in this session.")).toBeInTheDocument();
    expect(answerBox()).toHaveValue("js");
    expect(pressed("js jumpstream")).toBe("true");
  });

  it("stores the flags set with the buttons", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "Unsure" }));
    await userEvent.click(screen.getByRole("button", { name: "Right thumb" }));
    await userEvent.type(answerBox(), "lj{Enter}");

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

  it("keeps the window's flags across a reshape and resets them on the next window", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.click(screen.getByRole("button", { name: "Mixed" }));
    await userEvent.type(answerBox(), "n{Enter}");
    expect(await screen.findByText("00:03.000–00:07.000")).toBeInTheDocument();
    expect(pressed("Mixed")).toBe("true");

    await userEvent.type(answerBox(), "x{Enter}");
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toMatchObject([{ mixed: true }]);
    expect(pressed("Mixed")).toBe("false");
  });

  it("drops an inline thumb side when a thumb button is clicked, so the buttons show what is saved", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.type(answerBox(), "js tl");
    expect(pressed("Left thumb")).toBe("true");

    await userEvent.click(screen.getByRole("button", { name: "Left thumb" }));
    expect(answerBox()).toHaveValue("js");
    expect(pressed("Left thumb")).toBe("false");

    await userEvent.type(answerBox(), " tl");
    await userEvent.click(screen.getByRole("button", { name: "Right thumb" }));
    expect(answerBox()).toHaveValue("js");
    expect(pressed("Left thumb")).toBe("false");
    expect(pressed("Right thumb")).toBe("true");

    await userEvent.type(answerBox(), "{Enter}");
    await roundLoaded("Beta Song");
    expect(submitted(calls)).toMatchObject([{ thumbPref: "right" }]);
  });

  it("undoes the last saved label with u", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.type(answerBox(), "x{Enter}");
    await roundLoaded("Beta Song");

    await userEvent.type(answerBox(), "u{Enter}");

    await waitFor(() => {
      expect(argsOf(calls, "label_undo")).toEqual([{ eventId: "01EVENT1" }]);
    });
    expect(await screen.findByText("Undone: 1")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Beta Song" })).toBeInTheDocument();
  });

  it("says there is nothing to undo before any label is saved", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.type(answerBox(), "u{Enter}");
    expect(await screen.findByText("Nothing to undo in this session.")).toBeInTheDocument();
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
    await userEvent.type(answerBox(), "x{Enter}");
    await roundLoaded("Beta Song");

    await userEvent.type(answerBox(), "u{Enter}");
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("Undone: 0")).toBeInTheDocument();

    await userEvent.type(answerBox(), "u{Enter}");
    expect(await screen.findByText("Undone: 1")).toBeInTheDocument();
    expect(argsOf(calls, "label_undo")).toEqual([{ eventId: "01EVENT1" }, { eventId: "01EVENT1" }]);
  });

  it("ends the session with q and stops the section audio", async () => {
    renderScreen();
    await roundLoaded();
    const source = await playingSource();

    await userEvent.type(answerBox(), "q{Enter}");

    expect(await screen.findByText("Session ended.")).toBeInTheDocument();
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

    await userEvent.type(answerBox(), "x{Enter}");

    expect(await screen.findByText("Picking a window…")).toBeInTheDocument();
    expect(sampleRequests(calls)).toHaveLength(2);
    expect(source.stops).toBe(1);
  });

  it("reshapes with n and refetches the window", async () => {
    const calls = renderScreen();
    await roundLoaded();

    await userEvent.type(answerBox(), "n{Enter}");

    expect(await screen.findByText("00:03.000–00:07.000")).toBeInTheDocument();
    expect(argsOf(calls, "label_reshape")).toEqual([{ anchor: ANCHOR_A, op: "next" }]);
    await waitFor(() => {
      expect(argsOf(calls, "chart_window").at(-1)).toEqual({ md5: ANCHOR_A.md5, fromMs: 3000, toMs: 7000, layoutId: null });
    });
    expect(sampleRequests(calls)).toHaveLength(1);
    expect(argsOf(calls, "chart_audio")).toHaveLength(1);
  });

  it("words a parser error and keeps the round", async () => {
    const calls = renderScreen();
    await roundLoaded();
    await userEvent.type(answerBox(), "js s{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("“s” is a command");
    expect(submitted(calls)).toEqual([]);
  });

  it("words the service's unknown pattern error", async () => {
    renderScreen([WINDOW_A], {
      labelResolvePatterns: () => mockIpcError("INVALID_INPUT", { pattern: "zz" }, { messageKey: "label.error.unknown_pattern" }),
    });
    await roundLoaded();
    await userEvent.type(answerBox(), "zz{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("Unknown pattern “zz”.");
  });

  it("shows the done state when no window is left", async () => {
    renderScreen([null]);
    expect(await screen.findByText("No window left to label.")).toBeInTheDocument();
    expect(screen.queryByTestId("playfield")).not.toBeInTheDocument();
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
