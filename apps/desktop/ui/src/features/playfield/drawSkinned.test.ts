import { describe, expect, it } from "vitest";
import { COLUMN_COLORS } from "./colors";
import { DEFAULT_PLAYFIELD_THEME as T, drawSkinned } from "./draw";
import type { NoteSpan, Projection } from "./project";
import { DEFAULT_SKIN_LAYOUT_PARAMS, skinLayout } from "./skinLayout";
import { type LoadedSkin, SKIN_SLOT, type SkinImage, type SkinSlot } from "./skinModel";
import { recordingContext } from "./testCanvas";
import { image, skin7k } from "./testSkin";

// k = 2: columns are 60 px wide (column 3 is 80), the judgement line sits at y = 856.
const H = 960;
const IDENTITY = [1, 0, 0, 1, 0, 0];

function projection(spans: NoteSpan[]): Projection {
  return {
    notes: [],
    spans,
    beatLines: [],
    handSeparators: [],
    columns: Array.from({ length: 7 }, (_, i) => ({ x: i * 10, w: 10, hand: i < 3 ? "left" : "right" })),
    shade: [],
  };
}

function render(skin: LoadedSkin, spans: NoteSpan[]) {
  const layout = skinLayout(skin, H, 1);
  const rec = recordingContext();
  drawSkinned(rec.ctx, projection(spans), layout, skin, T, { width: layout.width, height: H, judgeY: layout.judgeY });
  return { ...rec, layout };
}

function withImages(images: [SkinSlot, SkinImage][], overrides: Partial<LoadedSkin> = {}): LoadedSkin {
  return skin7k(overrides, images);
}

const full = (img: SkinImage) => ({ sx: 0, sy: 0, sw: img.width, sh: img.height });

describe("drawSkinned notes", () => {
  it("draws a tap from note.{i} bottom-anchored at its head, and a column without one procedurally", () => {
    const note = image(100, 50);
    const { images, ops } = render(withImages([[SKIN_SLOT.note(0), note]]), [
      { col: 0, headY: 500, tailY: null },
      { col: 2, headY: 600, tailY: null },
    ]);
    expect(images).toEqual([
      { image: note.bitmap, ...full(note), dx: 0, dy: 458, dw: 60, dh: 42, alpha: 1, transform: IDENTITY },
    ]);
    const gap = T.noteGapPx;
    expect(ops.filter((op) => op.fill === COLUMN_COLORS.outer)).toEqual([
      { x: 120 + gap, y: 588, w: 60 - 2 * gap, h: 12, fill: COLUMN_COLORS.outer, alpha: 1 },
    ]);
  });

  it("draws an LN as body, flipped tail, then head, falling back to note.{i} for the head", () => {
    const note = image(100, 50);
    const tail = image(100, 20);
    const { images, all } = render(
      withImages([
        [SKIN_SLOT.note(0), note],
        [SKIN_SLOT.tail(0), tail],
      ]),
      [{ col: 0, headY: 800, tailY: 300 }],
    );
    const gap = T.noteGapPx;
    expect(all.filter((op) => op.type === "fill" && op.fill === COLUMN_COLORS.outer)).toEqual([
      { type: "fill", x: gap, y: 300, w: 60 - 2 * gap, h: 500, fill: COLUMN_COLORS.outer, alpha: T.lnBodyAlpha },
    ]);
    expect(images).toEqual([
      { image: tail.bitmap, ...full(tail), dx: 0, dy: 0, dw: 60, dh: 16.8, alpha: 1, transform: [1, 0, 0, -1, 0, 300] },
      { image: note.bitmap, ...full(note), dx: 0, dy: 758, dw: 60, dh: 42, alpha: 1, transform: IDENTITY },
    ]);
    const bodyAt = all.findIndex((op) => op.type === "fill" && op.alpha === T.lnBodyAlpha);
    const tailAt = all.findIndex((op) => op.type === "image" && op.image === tail.bitmap);
    const headAt = all.findIndex((op) => op.type === "image" && op.image === note.bitmap);
    expect(bodyAt).toBeLessThan(tailAt);
    expect(tailAt).toBeLessThan(headAt);
  });
});

describe("drawSkinned LN bodies", () => {
  // 30×30 in a 60-px column tiles every 60 px; the body is 150 px long.
  const body = image(30, 30);
  const bodyOps = (style: LoadedSkin["noteBodyStyle"], span: NoteSpan = { col: 0, headY: 500, tailY: 350 }) =>
    render(withImages([[SKIN_SLOT.body(0), body]], { noteBodyStyle: style }), [span]).images.filter(
      (op) => op.image === body.bitmap,
    );
  const tile = (dy: number, dh: number, sy: number, sh: number) => ({
    image: body.bitmap,
    sx: 0,
    sy,
    sw: 30,
    sh,
    dx: 0,
    dy,
    dw: 60,
    dh,
    alpha: 1,
    transform: IDENTITY,
  });

  it("stretches the whole image over the body for Stretch", () => {
    expect(bodyOps("Stretch")).toEqual([tile(350, 150, 0, 30)]);
  });

  it("tiles from the head end upward for RepeatBottom, cutting the top tile", () => {
    expect(bodyOps("RepeatBottom")).toEqual([tile(440, 60, 0, 30), tile(380, 60, 0, 30), tile(350, 30, 15, 15)]);
  });

  it("treats RepeatTopAndBottom as RepeatBottom", () => {
    expect(bodyOps("RepeatTopAndBottom")).toEqual(bodyOps("RepeatBottom"));
  });

  it("tiles from the tail end downward for RepeatTop, cutting the bottom tile", () => {
    expect(bodyOps("RepeatTop")).toEqual([tile(350, 60, 0, 30), tile(410, 60, 0, 30), tile(470, 30, 0, 15)]);
  });

  it("keeps the tile phase at the head but draws only the visible tiles", () => {
    const ops = bodyOps("RepeatBottom", { col: 0, headY: 1230, tailY: -500 });
    expect(ops).toHaveLength(17);
    expect(ops[0]).toEqual(tile(930, 30, 0, 15));
    expect(ops.at(-1)).toEqual(tile(0, 30, 15, 15));
  });

  it("stretches a body whose tiles would be thinner than the minimum", () => {
    const thin = image(300, 1);
    const ops = render(withImages([[SKIN_SLOT.body(0), thin]]), [{ col: 0, headY: 500, tailY: 350 }]).images;
    expect(60 / 300).toBeLessThan(DEFAULT_SKIN_LAYOUT_PARAMS.minBodyTilePx);
    expect(ops).toEqual([
      { image: thin.bitmap, ...full(thin), dx: 0, dy: 350, dw: 60, dh: 150, alpha: 1, transform: IDENTITY },
    ]);
  });
});

describe("drawSkinned stage", () => {
  it("fills column backgrounds with their skin colour and alpha", () => {
    const { ops } = render(
      skin7k({ colours: { column: [{ r: 10, g: 20, b: 30, a: 128 }], columnLine: null, judgementLine: null } }),
      [],
    );
    expect(ops).toContainEqual({ x: 0, y: 0, w: 60, h: H, fill: "rgb(10, 20, 30)", alpha: (128 / 255) ** 2 });
    expect(ops).toContainEqual({ x: 60, y: 0, w: 60, h: H, fill: "rgb(0, 0, 0)", alpha: 1 });
  });

  it("draws column lines down to the judgement line and the skin's judgement line", () => {
    const { ops, layout } = render(skin7k(), []);
    const lines = ops.filter((op) => op.fill === "rgb(255, 255, 255)" && op.h === 856);
    expect(lines.map((op) => op.x)).toEqual(layout.columnLines.map((l) => l.x));
    expect(ops).toContainEqual({ x: 0, y: 856, w: 460, h: 1.25, fill: "rgb(255, 255, 255)", alpha: 0.9 });
  });

  it("stretches stage-left and stage-right over the full height beside the columns", () => {
    const left = image(40, 100, 2);
    const right = image(16, 100);
    const { images } = render(
      withImages([
        [SKIN_SLOT.stageLeft, left],
        [SKIN_SLOT.stageRight, right],
      ]),
      [],
    );
    expect(images).toEqual([
      { image: left.bitmap, ...full(left), dx: 0, dy: 0, dw: 25, dh: H, alpha: 1, transform: IDENTITY },
      { image: right.bitmap, ...full(right), dx: 485, dy: 0, dw: 20, dh: H, alpha: 1, transform: IDENTITY },
    ]);
  });

  it("draws the hit target under the notes and drops the procedural judgement line it replaces", () => {
    const hint = image(100, 20);
    const note = image(100, 50);
    const { all, ops, layout } = render(
      withImages([
        [SKIN_SLOT.stageHint, hint],
        [SKIN_SLOT.note(0), note],
      ]),
      [{ col: 0, headY: 856, tailY: null }],
    );
    const target = layout.hitTarget;
    expect(target).not.toBeNull();
    const hintAt = all.findIndex((op) => op.type === "image" && op.image === hint.bitmap);
    expect(all[hintAt]).toMatchObject({ dx: target?.x, dy: target?.y, dw: target?.w, dh: target?.h });
    expect(hintAt).toBeLessThan(all.findIndex((op) => op.type === "image" && op.image === note.bitmap));
    expect(ops.some((op) => op.fill === T.judgementLine)).toBe(false);
  });

  it("falls back to the procedural judgement line on top when the skin has no hit target", () => {
    const { ops, layout } = render(skin7k(), []);
    expect(ops.at(-1)).toEqual({
      x: 0,
      y: layout.judgeY - T.judgementLinePx / 2,
      w: 460,
      h: T.judgementLinePx,
      fill: T.judgementLine,
      alpha: 1,
    });
  });

  it.each([
    [false, "over"],
    [true, "under"],
  ])("draws keys at the bottom with KeysUnderNotes=%s, %s the notes", (keysUnderNotes, where) => {
    const key = image(60, 200, 2);
    const note = image(100, 50);
    const { all } = render(
      withImages(
        [
          [SKIN_SLOT.key(0), key],
          [SKIN_SLOT.note(0), note],
        ],
        { keysUnderNotes },
      ),
      [{ col: 0, headY: 900, tailY: null }],
    );
    const keyAt = all.findIndex((op) => op.type === "image" && op.image === key.bitmap);
    const noteAt = all.findIndex((op) => op.type === "image" && op.image === note.bitmap);
    expect(all[keyAt]).toMatchObject({ dx: 0, dy: H - 200, dw: 60, dh: 200 });
    expect(keyAt < noteAt).toBe(where === "under");
  });
});
