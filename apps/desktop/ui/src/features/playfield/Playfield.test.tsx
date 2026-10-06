import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Clock } from "./audioClock";
import { DEFAULT_PLAYFIELD_THEME, draw } from "./draw";
import { DEFAULT_PLAYFIELD_VIEW, Playfield } from "./Playfield";
import { fitPxPerMs, project } from "./project";
import { type FillOp, recordingContext } from "./testCanvas";
import type { ChartWindow, ColumnHand } from "./types";

const HANDS: ColumnHand[] = ["left", "left", "left", "right", "right", "right", "right"];
const WINDOW: ChartWindow = {
  md5: "d41d8cd98f00b204e9800998ecf8427e",
  keymode: 7,
  fromMs: 1000,
  toMs: 2000,
  notes: [
    { tMs: 1000, col: 3, endMs: null },
    { tMs: 1500, col: 1, endMs: 1800 },
    { tMs: 2000, col: 0, endMs: null },
  ],
  timing: [{ tMs: 0, kind: "red", beatLenMs: 250, meter: 4, sv: null }],
  layout: { id: "k7.313_right_thumb", columns: HANDS.map((hand) => ({ hand, finger: "index" })) },
  chartSpan: { firstMs: 0, endMs: 60_000 },
  audioFilename: null,
};
const W = 700;
const H = 600;
const JUDGE_Y = 500;

function expectedOps(nowMs: number, pxPerMs: number, judgeY = JUDGE_Y): FillOp[] {
  const { ctx, ops } = recordingContext();
  const view = { width: W, height: H, judgeY };
  draw(ctx, project(WINDOW, { ...view, nowMs, pxPerMs }), DEFAULT_PLAYFIELD_THEME, view);
  return ops;
}

class FakeResizeObserver {
  static instances: FakeResizeObserver[] = [];
  targets: Element[] = [];
  disconnected = false;
  constructor(private readonly callback: ResizeObserverCallback) {
    FakeResizeObserver.instances.push(this);
  }
  observe(target: Element): void {
    this.targets.push(target);
  }
  unobserve(): void {
    this.targets = [];
  }
  disconnect(): void {
    this.disconnected = true;
  }
  resize(width: number, height: number): void {
    const entries = this.targets.map((target) => ({ target, contentRect: { width, height } }));
    act(() => {
      this.callback(entries as unknown as ResizeObserverEntry[], this);
    });
  }
}

let frames = new Map<number, FrameRequestCallback>();
let nextFrameId = 0;
const cancelled: number[] = [];

function runFrame(): void {
  const pending = [...frames.values()];
  frames = new Map();
  act(() => {
    for (const callback of pending) {
      callback(0);
    }
  });
}

function fakeClock(nowMs: number, playing: boolean): Clock & { nowMsValue: number; playing: boolean } {
  return {
    nowMsValue: nowMs,
    playing,
    nowMs() {
      return this.nowMsValue;
    },
    play: () => undefined,
    pause: () => undefined,
    dispose: () => undefined,
  };
}

let rec: ReturnType<typeof recordingContext>;

beforeEach(() => {
  FakeResizeObserver.instances = [];
  frames = new Map();
  nextFrameId = 0;
  cancelled.length = 0;
  rec = recordingContext();
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  vi.stubGlobal("devicePixelRatio", 2);
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    nextFrameId++;
    frames.set(nextFrameId, callback);
    return nextFrameId;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => {
    cancelled.push(id);
    frames.delete(id);
  });
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(rec.ctx as unknown as RenderingContext);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function observer(): FakeResizeObserver {
  const ro = FakeResizeObserver.instances.at(-1);
  if (ro === undefined) {
    throw new Error("Playfield did not observe its container");
  }
  return ro;
}

describe("Playfield", () => {
  it("draws nothing until the container has a size", () => {
    render(<Playfield window={WINDOW} clock={null} scroll="fit" judgeY={JUDGE_Y} />);
    expect(rec.ops).toEqual([]);
  });

  it("without a clock, draws the whole window once at its start with the fit speed, at device resolution", () => {
    const { container } = render(<Playfield window={WINDOW} clock={null} scroll={0.9} judgeY={JUDGE_Y} />);
    observer().resize(W, H);
    const canvas = container.querySelector("canvas");
    expect(canvas?.width).toBe(W * 2);
    expect(canvas?.height).toBe(H * 2);
    expect(rec.transforms.at(-1)).toEqual([2, 0, 0, 2, 0, 0]);
    expect(rec.ops).toEqual(expectedOps(WINDOW.fromMs, fitPxPerMs(WINDOW, H, JUDGE_Y)));
    expect(frames.size).toBe(0);
  });

  it("puts the judgement line near the bottom by default", () => {
    render(<Playfield window={WINDOW} clock={null} scroll="fit" />);
    observer().resize(W, H);
    const judgeY = H - DEFAULT_PLAYFIELD_VIEW.judgeInsetPx;
    expect(rec.ops).toEqual(expectedOps(WINDOW.fromMs, fitPxPerMs(WINDOW, H, judgeY), judgeY));
  });

  it("scrolls with the clock on every animation frame while it plays", () => {
    const clock = fakeClock(1200, true);
    render(<Playfield window={WINDOW} clock={clock} scroll={0.5} judgeY={JUDGE_Y} />);
    observer().resize(W, H);
    rec.ops.length = 0;
    runFrame();
    expect(rec.ops).toEqual(expectedOps(1200, 0.5));

    clock.nowMsValue = 1300;
    rec.ops.length = 0;
    runFrame();
    expect(rec.ops).toEqual(expectedOps(1300, 0.5));
  });

  it("uses the fit speed while playing when asked to", () => {
    const clock = fakeClock(1200, true);
    render(<Playfield window={WINDOW} clock={clock} scroll="fit" judgeY={JUDGE_Y} />);
    observer().resize(W, H);
    rec.ops.length = 0;
    runFrame();
    expect(rec.ops).toEqual(expectedOps(1200, fitPxPerMs(WINDOW, H, JUDGE_Y)));
  });

  it("shows the static window while the clock is paused and follows it once it plays", () => {
    const clock = fakeClock(1700, false);
    render(<Playfield window={WINDOW} clock={clock} scroll={0.5} judgeY={JUDGE_Y} />);
    observer().resize(W, H);
    runFrame();
    expect(rec.ops).toEqual(expectedOps(WINDOW.fromMs, fitPxPerMs(WINDOW, H, JUDGE_Y)));

    rec.ops.length = 0;
    runFrame();
    expect(rec.ops).toEqual([]);

    clock.playing = true;
    runFrame();
    expect(rec.ops).toEqual(expectedOps(1700, 0.5));
  });

  it("cancels the animation frame and the observer on unmount", () => {
    const clock = fakeClock(1200, true);
    const { unmount } = render(<Playfield window={WINDOW} clock={clock} scroll={0.5} judgeY={JUDGE_Y} />);
    const ro = observer();
    ro.resize(W, H);
    const pending = [...frames.keys()];
    expect(pending).toHaveLength(1);
    unmount();
    expect(cancelled).toEqual(pending);
    expect(ro.disconnected).toBe(true);
  });
});
