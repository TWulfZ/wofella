import { describe, expect, it } from "vitest";
import { DEFAULT_PLAYFIELD_PARAMS, fitPxPerMs, project, type ProjectView } from "./project";
import type { ChartWindow, ColumnHand } from "./types";

const HANDS_313: ColumnHand[] = ["left", "left", "left", "right", "right", "right", "right"];

function chartWindow(overrides: Partial<ChartWindow> = {}): ChartWindow {
  return {
    md5: "d41d8cd98f00b204e9800998ecf8427e",
    keymode: 7,
    fromMs: 900,
    toMs: 1800,
    notes: [],
    timing: [],
    layout: { id: "k7.313_right_thumb", columns: HANDS_313.map((hand) => ({ hand, finger: "index" })) },
    chartSpan: { firstMs: 0, endMs: 60_000 },
    audioFilename: null,
    ...overrides,
  };
}

// Visible times run from 800 ms (bottom, y = 600) to 2000 ms (top, y = 0).
const VIEW: ProjectView = { nowMs: 1000, pxPerMs: 0.5, width: 700, height: 600, judgeY: 500 };
const NOTE_H = DEFAULT_PLAYFIELD_PARAMS.noteHeightPx;
const TAIL_H = DEFAULT_PLAYFIELD_PARAMS.lnTailHeightPx;

describe("project", () => {
  it("lays out equal columns with a separator where the hand changes", () => {
    const out = project(chartWindow(), VIEW);
    expect(out.columns.map((c) => c.x)).toEqual([0, 100, 200, 300, 400, 500, 600]);
    expect(out.columns.every((c) => c.w === 100)).toBe(true);
    expect(out.columns.map((c) => c.hand)).toEqual(HANDS_313);
    expect(out.handSeparators).toEqual([{ x: 300 }]);
  });

  it("gives a both-thumbs column its own hand group", () => {
    const hands: ColumnHand[] = ["left", "left", "left", "both", "right", "right", "right"];
    const window = chartWindow({ layout: { id: "k7.both", columns: hands.map((hand) => ({ hand, finger: "thumb" })) } });
    expect(project(window, VIEW).handSeparators).toEqual([{ x: 300 }, { x: 400 }]);
  });

  it("scrolls taps down to the judgement line and drops the ones off screen", () => {
    const notes = [
      { tMs: 1000, col: 0, endMs: null },
      { tMs: 3000, col: 1, endMs: null },
      { tMs: 1990, col: 2, endMs: null },
    ];
    expect(project(chartWindow({ notes }), VIEW).notes).toEqual([
      { col: 0, x: 0, y: 500 - NOTE_H, w: 100, h: NOTE_H, kind: "tap", clipped: false },
      { col: 2, x: 200, y: 0, w: 100, h: 5, kind: "tap", clipped: true },
    ]);
  });

  it("draws an LN as body, tail and head, clipping the body to the visible area", () => {
    const notes = [
      { tMs: 1200, col: 4, endMs: 1400 },
      { tMs: 900, col: 3, endMs: 3000 },
    ];
    expect(project(chartWindow({ notes }), VIEW).notes).toEqual([
      { col: 4, x: 400, y: 300, w: 100, h: 100, kind: "lnBody", clipped: false },
      { col: 4, x: 400, y: 300 - TAIL_H, w: 100, h: TAIL_H, kind: "lnTail", clipped: false },
      { col: 4, x: 400, y: 400 - NOTE_H, w: 100, h: NOTE_H, kind: "lnHead", clipped: false },
      { col: 3, x: 300, y: 0, w: 100, h: 550, kind: "lnBody", clipped: true },
      { col: 3, x: 300, y: 550 - NOTE_H, w: 100, h: NOTE_H, kind: "lnHead", clipped: false },
    ]);
  });

  it("ignores notes in columns the layout does not have", () => {
    const notes = [{ tMs: 1000, col: 7, endMs: null }];
    expect(project(chartWindow({ notes }), VIEW).notes).toEqual([]);
  });

  it("draws beat lines from red lines, each one ending at the next, with measures by meter", () => {
    const timing = [
      { tMs: 0, kind: "red" as const, beatLenMs: 250, meter: 4, sv: null },
      { tMs: 1100, kind: "green" as const, beatLenMs: null, meter: null, sv: 0.5 },
      { tMs: 1600, kind: "red" as const, beatLenMs: 300, meter: 3, sv: null },
    ];
    expect(project(chartWindow({ timing }), VIEW).beatLines).toEqual([
      { y: 500, measure: true },
      { y: 375, measure: false },
      { y: 250, measure: false },
      { y: 200, measure: true },
      { y: 50, measure: false },
    ]);
  });

  it("falls back to the default meter and skips unusable beat lengths", () => {
    const timing = [
      { tMs: 0, kind: "red" as const, beatLenMs: 500, meter: null, sv: null },
      { tMs: 1700, kind: "red" as const, beatLenMs: 0, meter: 4, sv: null },
    ];
    expect(project(chartWindow({ timing }), VIEW).beatLines).toEqual([
      { y: 500, measure: false },
      { y: 250, measure: false },
    ]);
  });

  it("caps the number of beat lines", () => {
    const timing = [{ tMs: 0, kind: "red" as const, beatLenMs: 0.001, meter: 4, sv: null }];
    const out = project(chartWindow({ timing }), VIEW, { ...DEFAULT_PLAYFIELD_PARAMS, maxBeatLines: 10 });
    expect(out.beatLines).toHaveLength(10);
  });

  it("shades the visible regions outside [fromMs, toMs]", () => {
    expect(project(chartWindow(), VIEW).shade).toEqual([
      { y0: 0, y1: 100 },
      { y0: 550, y1: 600 },
    ]);
    expect(project(chartWindow({ fromMs: 0, toMs: 10_000 }), VIEW).shade).toEqual([]);
  });
});

describe("fitPxPerMs", () => {
  it("fits the window above the judgement line with room for the last note head", () => {
    const window = chartWindow({ fromMs: 1000, toMs: 5000 });
    expect(fitPxPerMs(window, 600, 500)).toBeCloseTo((500 - NOTE_H) / 4000);
    expect(fitPxPerMs(window, 300, 500)).toBeCloseTo((300 - NOTE_H) / 4000);
  });

  it("stays finite on an empty window", () => {
    const px = fitPxPerMs(chartWindow({ fromMs: 1000, toMs: 1000 }), 600, 500);
    expect(Number.isFinite(px)).toBe(true);
    expect(px).toBeGreaterThan(0);
  });
});
