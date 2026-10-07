import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Clock } from "./audioClock";
import { DEFAULT_PLAYFIELD_THEME, draw, drawSkinned } from "./draw";
import { Playfield } from "./Playfield";
import { fitPxPerMs, project } from "./project";
import { skinLayout } from "./skinLayout";
import { SKIN_SLOT } from "./skinModel";
import { DEFAULT_STAGE_PARAMS, judgeYFromHitPosition, osuPxPerMs, type ScrollMode } from "./stage";
import { fakeOffscreenCanvas, type FillOp, type Op, recordingContext } from "./testCanvas";
import { image, skin7k } from "./testSkin";
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
const CONTAINER_W = 700;
const H = 600;
// 30 virtual px per column, 7 columns, scaled from the 480-px space to H.
const STAGE_W = (30 * 7 * H) / 480;
// HitPosition 400 of 480 lands at y = 500 when H = 600.
const HIT_POSITION = 400;
const JUDGE_Y = 500;
const SPEED_20: ScrollMode = { kind: "osu", speed: 20 };
const PX: ScrollMode = { kind: "pxPerMs", value: 0.5 };

function expectedOps(nowMs: number, pxPerMs: number, judgeY = JUDGE_Y, width = STAGE_W): FillOp[] {
  const { ctx, ops } = recordingContext();
  const view = { width, height: H, judgeY };
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
    render(<Playfield window={WINDOW} clock={null} scroll="fit" hitPosition={HIT_POSITION} />);
    expect(rec.ops).toEqual([]);
  });

  it("without a clock, draws the window from its start at the judgement line at the chosen speed, at device resolution", () => {
    const { container } = render(<Playfield window={WINDOW} clock={null} scroll={SPEED_20} hitPosition={HIT_POSITION} />);
    observer().resize(CONTAINER_W, H);
    const canvas = container.querySelector("canvas");
    expect(canvas?.width).toBe(STAGE_W * 2);
    expect(canvas?.height).toBe(H * 2);
    expect(rec.transforms.at(-1)).toEqual([2, 0, 0, 2, 0, 0]);
    expect(rec.ops).toEqual(expectedOps(WINDOW.fromMs, osuPxPerMs(20, H)));
    expect(frames.size).toBe(0);
  });

  it("fits the whole window only when asked to", () => {
    render(<Playfield window={WINDOW} clock={null} scroll="fit" hitPosition={HIT_POSITION} />);
    observer().resize(CONTAINER_W, H);
    expect(rec.ops).toEqual(expectedOps(WINDOW.fromMs, fitPxPerMs(WINDOW, H, JUDGE_Y)));
  });

  it("puts the judgement line at the default HitPosition when none is given", () => {
    render(<Playfield window={WINDOW} clock={null} scroll={PX} />);
    observer().resize(CONTAINER_W, H);
    const judgeY = judgeYFromHitPosition(DEFAULT_STAGE_PARAMS.defaultHitPosition, H);
    expect(rec.ops).toEqual(expectedOps(WINDOW.fromMs, 0.5, judgeY));
  });

  it("sizes the stage from the columns and the height, centred, and scales it with the zoom", () => {
    const { container, rerender } = render(<Playfield window={WINDOW} clock={null} scroll={PX} hitPosition={HIT_POSITION} />);
    observer().resize(CONTAINER_W, H);
    const canvas = container.querySelector("canvas");
    expect(canvas?.style.width).toBe(`${STAGE_W}px`);
    expect(canvas?.style.left).toBe(`${(CONTAINER_W - STAGE_W) / 2}px`);

    rec.ops.length = 0;
    rerender(<Playfield window={WINDOW} clock={null} scroll={PX} hitPosition={HIT_POSITION} zoom={1.5} />);
    expect(canvas?.style.width).toBe(`${STAGE_W * 1.5}px`);
    expect(rec.ops).toEqual(expectedOps(WINDOW.fromMs, 0.5, JUDGE_Y, STAGE_W * 1.5));
  });

  it("uses the given column widths and never grows wider than its container", () => {
    const { container, rerender } = render(
      <Playfield window={WINDOW} clock={null} scroll={PX} hitPosition={HIT_POSITION} columnWidths={[42, 42, 42, 42, 42, 42, 42]} />,
    );
    observer().resize(CONTAINER_W, H);
    const canvas = container.querySelector("canvas");
    expect(canvas?.style.width).toBe(`${(42 * 7 * H) / 480}px`);

    rerender(<Playfield window={WINDOW} clock={null} scroll={PX} hitPosition={HIT_POSITION} zoom={2} columnWidths={[42, 42, 42, 42, 42, 42, 42]} />);
    expect(canvas?.style.width).toBe(`${CONTAINER_W}px`);
    expect(canvas?.style.left).toBe("0px");
  });

  it("scrolls with the clock on every animation frame while it plays", () => {
    const clock = fakeClock(1200, true);
    render(<Playfield window={WINDOW} clock={clock} scroll={SPEED_20} hitPosition={HIT_POSITION} />);
    observer().resize(CONTAINER_W, H);
    rec.ops.length = 0;
    runFrame();
    expect(rec.ops).toEqual(expectedOps(1200, osuPxPerMs(20, H)));

    clock.nowMsValue = 1300;
    rec.ops.length = 0;
    runFrame();
    expect(rec.ops).toEqual(expectedOps(1300, osuPxPerMs(20, H)));
  });

  it("divides the scroll velocity by the rate in map time", () => {
    const clock = fakeClock(1200, true);
    render(<Playfield window={WINDOW} clock={clock} scroll={PX} hitPosition={HIT_POSITION} rate={2} />);
    observer().resize(CONTAINER_W, H);
    rec.ops.length = 0;
    runFrame();
    expect(rec.ops).toEqual(expectedOps(1200, 0.25));
  });

  it("uses the fit speed while playing when asked to", () => {
    const clock = fakeClock(1200, true);
    render(<Playfield window={WINDOW} clock={clock} scroll="fit" hitPosition={HIT_POSITION} />);
    observer().resize(CONTAINER_W, H);
    rec.ops.length = 0;
    runFrame();
    expect(rec.ops).toEqual(expectedOps(1200, fitPxPerMs(WINDOW, H, JUDGE_Y)));
  });

  it("shows the window from its start at the chosen speed while paused and follows the clock once it plays", () => {
    const clock = fakeClock(1700, false);
    render(<Playfield window={WINDOW} clock={clock} scroll={PX} hitPosition={HIT_POSITION} />);
    observer().resize(CONTAINER_W, H);
    runFrame();
    expect(rec.ops).toEqual(expectedOps(WINDOW.fromMs, 0.5));

    rec.ops.length = 0;
    runFrame();
    expect(rec.ops).toEqual([]);

    clock.playing = true;
    runFrame();
    expect(rec.ops).toEqual(expectedOps(1700, 0.5));
  });

  it("cancels the animation frame and the observer on unmount", () => {
    const clock = fakeClock(1200, true);
    const { unmount } = render(<Playfield window={WINDOW} clock={clock} scroll={PX} hitPosition={HIT_POSITION} />);
    const ro = observer();
    ro.resize(CONTAINER_W, H);
    const pending = [...frames.keys()];
    expect(pending).toHaveLength(1);
    unmount();
    expect(cancelled).toEqual(pending);
    expect(ro.disconnected).toBe(true);
  });
});

describe("Playfield with a skin", () => {
  const SKIN = skin7k({}, [[SKIN_SLOT.note(3), image(100, 50)]]);
  let layerCanvases: ReturnType<typeof fakeOffscreenCanvas>;

  beforeEach(() => {
    layerCanvases = fakeOffscreenCanvas();
    vi.stubGlobal("OffscreenCanvas", layerCanvases.FakeOffscreenCanvas);
  });

  function expectedSkinned(nowMs: number, pxPerMs: number, zoom = 1) {
    const layout = skinLayout(SKIN, H, zoom);
    const { ctx, all } = recordingContext();
    const view = { width: Math.min(layout.width, CONTAINER_W), height: H, judgeY: layout.judgeY };
    // The component draws in CSS px under the device-pixel transform (devicePixelRatio 2 here).
    ctx.setTransform(2, 0, 0, 2, 0, 0);
    drawSkinned(ctx, project(WINDOW, { ...view, nowMs, pxPerMs }), layout, SKIN, DEFAULT_PLAYFIELD_THEME, view);
    return { all, layout };
  }

  /** The canvas's ops with each stage-layer blit replaced by what was painted on that layer. */
  function flattened(): Op[] {
    return rec.all.flatMap((op) => {
      const layer =
        op.type === "image" ? layerCanvases.instances.find((c) => (c as unknown) === op.image) : undefined;
      return layer === undefined ? [op] : layer.rec.all;
    });
  }

  it("draws with the skin, sized from its layout, and keeps zooming", () => {
    const { container, rerender } = render(
      <Playfield window={WINDOW} clock={null} scroll={PX} hitPosition={SKIN.hitPosition} columnWidths={SKIN.columnWidth} skin={SKIN} />,
    );
    observer().resize(CONTAINER_W, H);
    const { all, layout } = expectedSkinned(WINDOW.fromMs, 0.5);
    const canvas = container.querySelector("canvas");
    expect(canvas?.style.width).toBe(`${layout.width}px`);
    expect(flattened()).toEqual(all);
    expect(rec.images.length).toBeGreaterThan(0);

    rec.all.length = 0;
    rerender(
      <Playfield
        window={WINDOW}
        clock={null}
        scroll={PX}
        hitPosition={SKIN.hitPosition}
        columnWidths={SKIN.columnWidth}
        skin={SKIN}
        zoom={1.25}
      />,
    );
    const zoomed = expectedSkinned(WINDOW.fromMs, 0.5, 1.25);
    expect(canvas?.style.width).toBe(`${zoomed.layout.width}px`);
    expect(flattened()).toEqual(zoomed.all);
  });

  it("paints the stage background once while playing and again once after a resize", () => {
    const clock = fakeClock(1200, true);
    render(<Playfield window={WINDOW} clock={clock} scroll={PX} skin={SKIN} />);
    const ro = observer();
    ro.resize(CONTAINER_W, H);
    runFrame();
    clock.nowMsValue = 1300;
    runFrame();
    expect(layerCanvases.instances.map((c) => [c.width, c.height])).toEqual([
      [Math.round(Math.min(skinLayout(SKIN, H, 1).width, CONTAINER_W) * 2), H * 2],
    ]);
    ro.resize(CONTAINER_W, H + 100);
    runFrame();
    runFrame();
    expect(layerCanvases.instances).toHaveLength(2);
    // The replaced background gives its memory back.
    expect(layerCanvases.instances[0]?.width).toBe(0);
  });

  it("releases the cached background when the skin becomes null", () => {
    const { rerender } = render(<Playfield window={WINDOW} clock={null} scroll={PX} skin={SKIN} />);
    observer().resize(CONTAINER_W, H);
    expect(layerCanvases.instances).toHaveLength(1);
    expect(layerCanvases.instances[0]?.width).toBeGreaterThan(0);
    rerender(<Playfield window={WINDOW} clock={null} scroll={PX} skin={null} />);
    expect(layerCanvases.instances[0]?.width).toBe(0);
  });

  it.each([402, 428, 465])(
    "lands the skin's notes on HitPosition %i when their time comes, at stable's travel time, and hides those past it",
    (hitPosition) => {
      const noteBitmap = image(100, 50);
      const skin = skin7k({ hitPosition }, [[SKIN_SLOT.note(3), noteBitmap]]);
      const speed = 30;
      const travelMs = (hitPosition * 200) / (7 * speed);
      const notes = [
        { tMs: 1000, col: 3, endMs: null },
        { tMs: 1000 + travelMs / 2, col: 3, endMs: null },
        { tMs: 990, col: 3, endMs: null },
      ];
      const clock = fakeClock(1000, true);
      render(<Playfield window={{ ...WINDOW, notes }} clock={clock} scroll={{ kind: "osu", speed }} skin={skin} />);
      observer().resize(CONTAINER_W, H);
      rec.images.length = 0;
      runFrame();
      const judgeY = skinLayout(skin, H, 1).judgeY;
      expect(judgeY).toBeCloseTo((hitPosition * H) / 480);
      const bottoms = rec.images.filter((op) => op.image === noteBitmap.bitmap).map((op) => op.dy + op.dh);
      expect(bottoms).toHaveLength(2);
      expect(bottoms[0]).toBeCloseTo(judgeY);
      expect(bottoms[1]).toBeCloseTo(judgeY / 2);
    },
  );

  it("draws procedurally, with no image, when the skin is null", () => {
    render(<Playfield window={WINDOW} clock={null} scroll={PX} hitPosition={HIT_POSITION} skin={null} />);
    observer().resize(CONTAINER_W, H);
    expect(rec.images).toEqual([]);
    expect(rec.ops).toEqual(expectedOps(WINDOW.fromMs, 0.5));
  });
});
