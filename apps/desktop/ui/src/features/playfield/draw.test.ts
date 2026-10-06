import { describe, expect, it } from "vitest";
import { COLUMN_COLORS } from "./colors";
import { DEFAULT_PLAYFIELD_THEME as T, draw } from "./draw";
import type { Projection } from "./project";
import { recordingContext } from "./testCanvas";

const VIEW = { width: 300, height: 600, judgeY: 500 };

function projection(overrides: Partial<Projection> = {}): Projection {
  return {
    notes: [],
    beatLines: [],
    handSeparators: [{ x: 200 }],
    columns: [
      { x: 0, w: 100, hand: "left" },
      { x: 100, w: 100, hand: "left" },
      { x: 200, w: 100, hand: "right" },
    ],
    shade: [],
    ...overrides,
  };
}

function render(p: Projection) {
  const { ctx, ops } = recordingContext();
  draw(ctx, p, T, VIEW);
  return ops;
}

describe("draw", () => {
  it("paints the background and tints every column", () => {
    const ops = render(projection());
    expect(ops[0]).toEqual({ x: 0, y: 0, w: 300, h: 600, fill: T.background, alpha: 1 });
    expect(ops.slice(1, 4)).toEqual(
      [0, 100, 200].map((x) => ({ x, y: 0, w: 100, h: 600, fill: T.columnTint, alpha: 1 })),
    );
  });

  it("separates columns with thin lines and hands with a thicker one", () => {
    const ops = render(projection());
    expect(T.handSeparatorPx).toBeGreaterThan(T.columnSeparatorPx);
    expect(ops.filter((op) => op.fill === T.columnSeparator)).toEqual([
      { x: 100 - T.columnSeparatorPx / 2, y: 0, w: T.columnSeparatorPx, h: 600, fill: T.columnSeparator, alpha: 1 },
    ]);
    expect(ops.filter((op) => op.fill === T.handSeparator)).toEqual([
      { x: 200 - T.handSeparatorPx / 2, y: 0, w: T.handSeparatorPx, h: 600, fill: T.handSeparator, alpha: 1 },
    ]);
  });

  it("draws measure lines bolder than beat lines", () => {
    const ops = render(
      projection({
        beatLines: [
          { y: 300, measure: true },
          { y: 400, measure: false },
        ],
      }),
    );
    expect(ops.filter((op) => op.fill === T.measureLine)).toEqual([
      { x: 0, y: 300 - T.measureLinePx / 2, w: 300, h: T.measureLinePx, fill: T.measureLine, alpha: 1 },
    ]);
    expect(ops.filter((op) => op.fill === T.beatLine)).toEqual([
      { x: 0, y: 400 - T.beatLinePx / 2, w: 300, h: T.beatLinePx, fill: T.beatLine, alpha: 1 },
    ]);
  });

  it("colours notes by column, with translucent LN bodies", () => {
    const gap = T.noteGapPx;
    const ops = render(
      projection({
        notes: [
          { col: 1, x: 100, y: 200, w: 100, h: 12, kind: "tap", clipped: false },
          { col: 0, x: 0, y: 100, w: 100, h: 88, kind: "lnBody", clipped: false },
          { col: 0, x: 0, y: 94, w: 100, h: 6, kind: "lnTail", clipped: false },
          { col: 0, x: 0, y: 176, w: 100, h: 12, kind: "lnHead", clipped: false },
        ],
      }),
    );
    const noteOps = ops.filter((op) => op.fill === COLUMN_COLORS.centre || op.fill === COLUMN_COLORS.outer);
    expect(noteOps).toEqual([
      { x: 100 + gap, y: 200, w: 100 - 2 * gap, h: 12, fill: COLUMN_COLORS.centre, alpha: 1 },
      { x: gap, y: 100, w: 100 - 2 * gap, h: 88, fill: COLUMN_COLORS.outer, alpha: T.lnBodyAlpha },
      { x: gap, y: 94, w: 100 - 2 * gap, h: 6, fill: COLUMN_COLORS.outer, alpha: 1 },
      { x: gap, y: 176, w: 100 - 2 * gap, h: 12, fill: COLUMN_COLORS.outer, alpha: 1 },
    ]);
  });

  it("shades outside the window over the notes and ends with the judgement line", () => {
    const ops = render(
      projection({
        notes: [{ col: 2, x: 200, y: 50, w: 100, h: 12, kind: "tap", clipped: false }],
        shade: [{ y0: 0, y1: 100 }],
      }),
    );
    const noteAt = ops.findIndex((op) => op.y === 50);
    const shadeAt = ops.findIndex((op) => op.fill === T.shade);
    expect(ops[shadeAt]).toEqual({ x: 0, y: 0, w: 300, h: 100, fill: T.shade, alpha: 1 });
    expect(shadeAt).toBeGreaterThan(noteAt);
    expect(ops.at(-1)).toEqual({
      x: 0,
      y: 500 - T.judgementLinePx / 2,
      w: 300,
      h: T.judgementLinePx,
      fill: T.judgementLine,
      alpha: 1,
    });
  });
});
