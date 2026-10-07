import { describe, expect, it } from "vitest";
import { DEFAULT_PLAYFIELD_PARAMS, fitPxPerMs, project, type ProjectView } from "./project";
import { judgeYFromHitPosition, osuPxPerMs } from "./stage";
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

  it("draws an LN as body, tail and head, holding a started one's head on the judgement line", () => {
    const notes = [
      { tMs: 1200, col: 4, endMs: 1400 },
      { tMs: 900, col: 3, endMs: 3000 },
    ];
    expect(project(chartWindow({ notes }), VIEW).notes).toEqual([
      { col: 4, x: 400, y: 300, w: 100, h: 100, kind: "lnBody", clipped: false },
      { col: 4, x: 400, y: 300 - TAIL_H, w: 100, h: TAIL_H, kind: "lnTail", clipped: false },
      { col: 4, x: 400, y: 400 - NOTE_H, w: 100, h: NOTE_H, kind: "lnHead", clipped: false },
      { col: 3, x: 300, y: 0, w: 100, h: 500, kind: "lnBody", clipped: true },
      { col: 3, x: 300, y: 500 - NOTE_H, w: 100, h: NOTE_H, kind: "lnHead", clipped: false },
    ]);
  });

  it("keeps each note's head and tail y for skinned drawing, uncut by the top edge, dropping notes entirely above it", () => {
    const notes = [
      { tMs: 1000, col: 0, endMs: null },
      { tMs: 900, col: 3, endMs: 3000 },
      { tMs: 2100, col: 1, endMs: null },
      { tMs: 1000, col: 7, endMs: null },
    ];
    expect(project(chartWindow({ notes }), VIEW).spans).toEqual([
      { col: 0, headY: 500, tailY: null },
      { col: 3, headY: 500, tailY: -500 },
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

  it("shades the regions above the judgement line outside [fromMs, toMs]", () => {
    expect(project(chartWindow(), VIEW).shade).toEqual([{ y0: 0, y1: 100 }]);
    expect(project(chartWindow({ fromMs: 1100 }), VIEW).shade).toEqual([
      { y0: 0, y1: 100 },
      { y0: 450, y1: 500 },
    ]);
    expect(project(chartWindow({ fromMs: 0, toMs: 10_000 }), VIEW).shade).toEqual([]);
    expect(project(chartWindow({ fromMs: 0, toMs: 500 }), VIEW).shade).toEqual([{ y0: 0, y1: 500 }]);
  });
});

// osu! judges a note when it reaches the skin's HitPosition; what passed it is hit or held, never drawn lower.
describe("the judgement line ends every note's travel", () => {
  const H = 600;
  const viewAt = (hitPosition: number, nowMs = 1000): ProjectView => ({
    nowMs,
    pxPerMs: osuPxPerMs(30, H),
    width: 700,
    height: H,
    judgeY: judgeYFromHitPosition(hitPosition, H),
  });

  it.each([240, 402, 428, 465, 480])("lands a note whose time is now with its bottom edge on HitPosition %i", (hitPosition) => {
    const view = viewAt(hitPosition);
    const out = project(chartWindow({ notes: [{ tMs: 1000, col: 2, endMs: null }] }), view);
    const tap = out.notes[0];
    expect(tap?.kind).toBe("tap");
    expect((tap?.y ?? 0) + (tap?.h ?? 0)).toBe(view.judgeY);
    expect(out.spans).toEqual([{ col: 2, headY: view.judgeY, tailY: null }]);
  });

  it.each([240, 402, 428, 480])("draws no tap that has passed HitPosition %i", (hitPosition) => {
    const notes = [
      { tMs: 999, col: 0, endMs: null },
      { tMs: 950, col: 1, endMs: null },
      { tMs: 1001, col: 2, endMs: null },
    ];
    const out = project(chartWindow({ notes }), viewAt(hitPosition));
    expect(out.notes.map((n) => n.col)).toEqual([2]);
    expect(out.spans.map((s) => s.col)).toEqual([2]);
  });

  it.each([240, 402, 428, 480])("holds a started LN's head on HitPosition %i and cuts its body there", (hitPosition) => {
    const view = viewAt(hitPosition);
    const tailY = view.judgeY - 200 * view.pxPerMs;
    const out = project(chartWindow({ notes: [{ tMs: 900, col: 3, endMs: 1200 }] }), view);
    expect(out.spans).toEqual([{ col: 3, headY: view.judgeY, tailY }]);
    const body = out.notes.find((n) => n.kind === "lnBody");
    const head = out.notes.find((n) => n.kind === "lnHead");
    expect(body?.y).toBeCloseTo(tailY);
    expect((body?.y ?? 0) + (body?.h ?? 0)).toBeCloseTo(view.judgeY);
    expect((head?.y ?? 0) + (head?.h ?? 0)).toBe(view.judgeY);
  });

  it("drops an LN once its tail has passed the line", () => {
    const out = project(chartWindow({ notes: [{ tMs: 800, col: 3, endMs: 999 }] }), viewAt(428));
    expect(out.notes).toEqual([]);
    expect(out.spans).toEqual([]);
  });

  it("draws neither notes nor beat lines below the line", () => {
    const view = viewAt(428, 1234);
    const notes = Array.from({ length: 40 }, (_, i) => ({
      tMs: 800 + i * 25,
      col: i % 7,
      endMs: i % 3 === 0 ? 800 + i * 25 + 120 : null,
    }));
    const timing = [{ tMs: 0, kind: "red" as const, beatLenMs: 50, meter: 4, sv: null }];
    const out = project(chartWindow({ notes, timing }), view);
    expect(out.notes.length).toBeGreaterThan(0);
    for (const n of out.notes) {
      expect(n.y + n.h).toBeLessThanOrEqual(view.judgeY + 1e-9);
    }
    for (const s of out.spans) {
      expect(s.headY).toBeLessThanOrEqual(view.judgeY + 1e-9);
    }
    expect(out.beatLines.length).toBeGreaterThan(0);
    for (const line of out.beatLines) {
      expect(line.y).toBeLessThanOrEqual(view.judgeY + 1e-9);
    }
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
