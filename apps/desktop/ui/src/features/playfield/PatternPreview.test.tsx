import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { columnColor } from "./colors";
import { PatternPreview } from "./PatternPreview";
import { type FillOp, recordingContext } from "./testCanvas";
import type { ChartNote, ChartWindow, ColumnHand } from "./types";

const HANDS: ColumnHand[] = ["left", "left", "left", "right", "right", "right", "right"];
const W = 120;
const H = 160;
const COL_W = W / HANDS.length;

function exampleWindow(notes: ChartNote[]): ChartWindow {
  return {
    md5: "",
    keymode: 7,
    fromMs: 0,
    toMs: 1000,
    notes,
    timing: [{ tMs: 0, kind: "red", beatLenMs: 250, meter: 4, sv: null }],
    layout: { id: "k7.313_right_thumb", columns: HANDS.map((hand) => ({ hand, finger: "index" })) },
    chartSpan: { firstMs: 0, endMs: 1000 },
    audioFilename: null,
  };
}

const MINIJACK = exampleWindow([
  { tMs: 0, col: 3, endMs: null },
  { tMs: 1000, col: 3, endMs: null },
]);
const LN = exampleWindow([{ tMs: 0, col: 0, endMs: 1000 }]);

function noteFills(ops: FillOp[], col: number): FillOp[] {
  const color = columnColor(7, col);
  return ops.filter((op) => op.fill === color && op.x >= col * COL_W && op.x + op.w <= (col + 1) * COL_W + 1e-9);
}

let rec = recordingContext();
const raf = vi.fn(() => 0);

beforeEach(() => {
  rec = recordingContext();
  raf.mockClear();
  vi.stubGlobal("devicePixelRatio", 2);
  vi.stubGlobal("requestAnimationFrame", raf);
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(rec.ctx as unknown as RenderingContext);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("PatternPreview", () => {
  it("draws the whole window once at device resolution, without a clock", () => {
    render(<PatternPreview window={MINIJACK} width={W} height={H} label="Example of minijack" />);

    const canvas = screen.getByRole<HTMLCanvasElement>("img", { name: "Example of minijack" });
    expect(canvas.tagName).toBe("CANVAS");
    expect([canvas.width, canvas.height]).toEqual([W * 2, H * 2]);
    expect(rec.transforms).toEqual([[2, 0, 0, 2, 0, 0]]);
    expect(raf).not.toHaveBeenCalled();

    const notes = noteFills(rec.ops, 3);
    expect(notes).toHaveLength(2);
    // Paused at fromMs: the first note rests on the bottom edge, the last one reaches the top.
    const ys = notes.map((op) => op.y).sort((a, b) => a - b);
    expect(ys[0]).toBeLessThan(H * 0.1);
    expect((ys[1] ?? 0) + (notes[0]?.h ?? 0)).toBeGreaterThan(H * 0.9);
  });

  it("keeps notes thin at preview size", () => {
    render(<PatternPreview window={MINIJACK} width={W} height={H} label="minijack" />);

    const notes = noteFills(rec.ops, 3);
    expect(notes).toHaveLength(2);
    for (const op of notes) {
      expect(op.h).toBeGreaterThanOrEqual(3);
      expect(op.h).toBeLessThanOrEqual(5);
    }
  });

  it("redraws when the window changes", () => {
    const { rerender } = render(<PatternPreview window={MINIJACK} width={W} height={H} label="example" />);
    const before = rec.ops.length;

    rerender(<PatternPreview window={LN} width={W} height={H} label="example" />);

    const after = rec.ops.slice(before);
    expect(after.length).toBeGreaterThan(0);
    expect(noteFills(after, 3)).toHaveLength(0);
    // Body, tail and head of the long note.
    expect(noteFills(after, 0)).toHaveLength(3);
  });
});
