import { describe, expect, it, vi } from "vitest";
import { COLUMN_COLORS } from "./colors";
import { DEFAULT_PLAYFIELD_THEME as T, drawSkinned } from "./draw";
import type { SurfaceFactory } from "./offscreen";
import type { NoteSpan, Projection } from "./project";
import { DEFAULT_SKIN_LAYOUT_PARAMS, skinLayout } from "./skinLayout";
import { type LoadedSkin, SKIN_SLOT, type SkinImage, type SkinSlot } from "./skinModel";
import { createStageBackground } from "./stageLayers";
import { type Op, recordingContext } from "./testCanvas";
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

  it("draws an LN as body, the pre-flipped tail above its time, then head, falling back to note.{i} for the head", () => {
    const note = image(100, 50);
    const tail = image(100, 20);
    const skin = withImages([
      [SKIN_SLOT.note(0), note],
      [SKIN_SLOT.tail(0), tail],
    ]);
    const flipped = skin.lnTails.get(0);
    expect(flipped?.bitmap).not.toBe(tail.bitmap);
    const { images, all } = render(skin, [{ col: 0, headY: 800, tailY: 300 }]);
    const gap = T.noteGapPx;
    expect(all.filter((op) => op.type === "fill" && op.fill === COLUMN_COLORS.outer)).toEqual([
      { type: "fill", x: gap, y: 300, w: 60 - 2 * gap, h: 500, fill: COLUMN_COLORS.outer, alpha: T.lnBodyAlpha },
    ]);
    expect(images).toEqual([
      { image: flipped?.bitmap, ...full(tail), dx: 0, dy: 283.2, dw: 60, dh: 16.8, alpha: 1, transform: IDENTITY },
      { image: note.bitmap, ...full(note), dx: 0, dy: 758, dw: 60, dh: 42, alpha: 1, transform: IDENTITY },
    ]);
    const bodyAt = all.findIndex((op) => op.type === "fill" && op.alpha === T.lnBodyAlpha);
    const tailAt = all.findIndex((op) => op.type === "image" && op.image === flipped?.bitmap);
    const headAt = all.findIndex((op) => op.type === "image" && op.image === note.bitmap);
    expect(bodyAt).toBeLessThan(tailAt);
    expect(tailAt).toBeLessThan(headAt);
  });

  it("skips a tail image thinner than half a pixel, like percy's 1-row transparent tail", () => {
    const note = image(100, 50);
    // 1 row over 256 px at the 84-px note width is 0.33 px tall.
    const thin = image(256, 1);
    const skin = withImages([
      [SKIN_SLOT.note(0), note],
      [SKIN_SLOT.tail(0), thin],
    ]);
    expect(84 / 256).toBeLessThan(DEFAULT_SKIN_LAYOUT_PARAMS.minImageDrawPx);
    const { images } = render(skin, [{ col: 0, headY: 800, tailY: 300 }]);
    expect(images.map((op) => op.image)).toEqual([note.bitmap]);
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

  it.each(["RepeatBottom", "RepeatTop", "RepeatTopAndBottom"] as const)(
    "anchors %s at the tail: row 0 at the tail, tiles toward the head, the last one cut at the head",
    (style) => {
      expect(bodyOps(style)).toEqual([tile(350, 60, 0, 30), tile(410, 60, 0, 30), tile(470, 30, 0, 15)]);
    },
  );

  it("keeps the tile phase at the tail but draws only the visible tiles", () => {
    const ops = bodyOps("RepeatBottom", { col: 0, headY: 1230, tailY: -500 });
    expect(ops).toHaveLength(17);
    expect(ops[0]).toEqual(tile(0, 40, 10, 20));
    expect(ops.at(-1)).toEqual(tile(940, 20, 0, 10));
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

describe("drawSkinned strip bodies", () => {
  // A percy-style body: transparent lead-in and cap in its top rows, then a plain bar. In a 60-px column a
  // 138-px-wide image shows at 60/138 of its height; the cropped 4096 rows of 40000 span about 1781 px.
  const cap = 4096;
  const source = 40_000;
  const tall = image(138, cap, 1, source);
  const tallH = (cap * 60) / 138;
  const stripOps = (img: SkinImage, span: NoteSpan, style: LoadedSkin["noteBodyStyle"] = "RepeatBottom") =>
    render(withImages([[SKIN_SLOT.body(0), img]], { noteBodyStyle: style }), [span]).images.filter(
      (op) => op.image === img.bitmap,
    );
  const rows = DEFAULT_SKIN_LAYOUT_PARAMS.stripTailRows;

  it("draws a truncated body once from the tail, its source starting at row 0", () => {
    const ops = stripOps(tall, { col: 0, headY: 856, tailY: 100 });
    expect(ops).toHaveLength(1);
    expect(ops[0]).toMatchObject({ sx: 0, sy: 0, sw: 138, dx: 0, dy: 100, dw: 60, dh: 756 });
    expect(ops[0]?.sh).toBeCloseTo((756 / tallH) * cap);
  });

  it("stretches the last source rows over a hold longer than the art instead of tiling it again", () => {
    const short = image(138, 400, 1, 4000);
    const artH = (400 * 60) / 138;
    const ops = stripOps(short, { col: 0, headY: 856, tailY: 100 });
    expect(ops).toHaveLength(2);
    expect(ops[0]).toMatchObject({ sy: 0, sh: 400, dy: 100 });
    expect(ops[0]?.dh).toBeCloseTo(artH);
    expect(ops[1]).toMatchObject({ sy: 400 - rows, sh: rows });
    expect(ops[1]?.dy).toBeCloseTo(100 + artH);
    expect((ops[1]?.dy ?? 0) + (ops[1]?.dh ?? 0)).toBeCloseTo(856);
  });

  it("never brings the art's top rows back on a hold many times longer than the art", () => {
    const short = image(138, 400, 1, 4000);
    const ops = stripOps(short, { col: 0, headY: 950, tailY: -20_000 });
    expect(ops).toHaveLength(1);
    expect(ops[0]).toMatchObject({ sy: 400 - rows, sh: rows, dy: 0, dh: 950 });
  });

  it.each(["RepeatBottom", "RepeatTop", "RepeatTopAndBottom"] as const)(
    "treats an untruncated strip-shaped body as one strip for %s",
    (style) => {
      const strip = image(138, 1380);
      expect(1380 / 138).toBeGreaterThanOrEqual(DEFAULT_SKIN_LAYOUT_PARAMS.stripBodyAspect);
      const ops = stripOps(strip, { col: 0, headY: 950, tailY: 0 }, style);
      expect(ops).toHaveLength(2);
      expect(ops[0]).toMatchObject({ sy: 0, sh: 1380, dy: 0, dh: 600 });
      expect(ops[1]).toMatchObject({ sy: 1380 - rows, sh: rows, dy: 600, dh: 350 });
    },
  );

  it("keeps a cropped Stretch body's proportions: the kept rows over their share of the hold, the rest stretched", () => {
    const ops = stripOps(tall, { col: 0, headY: 856, tailY: 100 }, "Stretch");
    const artH = (756 * cap) / source;
    expect(ops).toHaveLength(2);
    expect(ops[0]).toMatchObject({ sx: 0, sy: 0, sw: 138, sh: cap, dx: 0, dy: 100, dw: 60 });
    expect(ops[0]?.dh).toBeCloseTo(artH);
    expect(ops[1]).toMatchObject({ sy: cap - rows, sh: rows });
    expect(ops[1]?.dy).toBeCloseTo(100 + artH);
    expect((ops[1]?.dy ?? 0) + (ops[1]?.dh ?? 0)).toBeCloseTo(856);
  });

  it("still stretches an uncropped body over the whole hold for Stretch, strip-shaped or not", () => {
    const strip = image(138, 1380);
    const ops = stripOps(strip, { col: 0, headY: 856, tailY: 100 }, "Stretch");
    expect(ops).toEqual([expect.objectContaining({ sy: 0, sh: 1380, dy: 100, dh: 756 })]);
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

describe("drawSkinned stage layers", () => {
  const SKIN = withImages(
    [
      [SKIN_SLOT.stageLeft, image(40, 100, 2)],
      [SKIN_SLOT.stageRight, image(16, 100)],
      [SKIN_SLOT.stageHint, image(100, 20)],
      [SKIN_SLOT.stageBottom, image(200, 20)],
      [SKIN_SLOT.key(0), image(60, 200, 2)],
      [SKIN_SLOT.note(0), image(100, 50)],
    ],
    { colours: { column: [{ r: 10, g: 20, b: 30, a: 128 }], columnLine: null, judgementLine: null } },
  );
  const SPANS: NoteSpan[] = [
    { col: 0, headY: 500, tailY: null },
    { col: 1, headY: 700, tailY: 300 },
  ];
  const PROJECTION: Projection = {
    ...projection(SPANS),
    beatLines: [
      { y: 200, measure: true },
      { y: 400, measure: false },
    ],
    shade: [{ y0: 900, y1: 960 }],
  };

  function surfaces(fail = false) {
    const made: { image: object; w: number; h: number; rec: ReturnType<typeof recordingContext> }[] = [];
    const factory = vi.fn<SurfaceFactory>((w, h) => {
      if (fail) {
        return null;
      }
      const rec = recordingContext();
      const surface = { image: { surface: made.length }, w, h, rec };
      made.push(surface);
      return { image: surface.image as unknown as CanvasImageSource, ctx: rec.ctx, release: vi.fn() };
    });
    return { made, factory, layers: createStageBackground(factory) };
  }

  function frame(layout: ReturnType<typeof skinLayout>, layers: ReturnType<typeof createStageBackground> | null, dpr = 1) {
    const rec = recordingContext();
    const view = { width: layout.width, height: layout.height, judgeY: layout.judgeY };
    drawSkinned(rec.ctx, PROJECTION, layout, SKIN, T, view, undefined, layers === null ? undefined : { background: layers, dpr });
    return rec;
  }

  /** The main canvas's ops with every layer blit replaced by what was painted on that layer. */
  function flatten(all: Op[], made: ReturnType<typeof surfaces>["made"]): Op[] {
    return all.flatMap((op) => {
      const layer = op.type === "image" ? made.find((m) => m.image === op.image) : undefined;
      return layer === undefined ? [op] : layer.rec.all;
    });
  }

  it("composes the same picture as drawing the stage directly", () => {
    const layout = skinLayout(SKIN, H, 1);
    const { made, layers } = surfaces();
    const layered = frame(layout, layers);
    expect(made).toHaveLength(1);
    expect(flatten(layered.all, made)).toEqual(frame(layout, null).all);
  });

  it("caches only the opaque background: stage, stage sides and column backgrounds", () => {
    const layout = skinLayout(SKIN, H, 1);
    const { made, layers } = surfaces();
    frame(layout, layers);
    const painted = made[0]?.rec;
    expect(painted?.images.map((op) => op.image)).toEqual([
      SKIN.images.get(SKIN_SLOT.stageLeft)?.bitmap,
      SKIN.images.get(SKIN_SLOT.stageRight)?.bitmap,
    ]);
    expect(painted?.ops.map((op) => op.fill)).toEqual([T.background, ...layout.columns.map((c) => c.background.style)]);
  });

  it("paints the background once and draws lines, hit target, keys and stage bottom on every frame", () => {
    const layout = skinLayout(SKIN, H, 1);
    const { made, factory, layers } = surfaces();
    const first = frame(layout, layers);
    const painted = made.map((m) => m.rec.all.length);
    const second = frame(layout, layers);
    expect(factory).toHaveBeenCalledTimes(1);
    expect(made.map((m) => m.rec.all.length)).toEqual(painted);
    expect(second.all).toEqual(first.all);
    expect(second.images[0]).toMatchObject({
      image: made[0]?.image,
      sx: 0,
      sy: 0,
      sw: made[0]?.w,
      sh: made[0]?.h,
      dx: 0,
      dy: 0,
    });
    // The background is never repainted on the main canvas.
    expect(second.ops.some((op) => op.fill === "rgb(10, 20, 30)" || op.fill === T.background)).toBe(false);
    const drawn = second.images.map((op) => op.image);
    expect(drawn).not.toContain(SKIN.images.get(SKIN_SLOT.stageLeft)?.bitmap);
    expect(drawn).not.toContain(SKIN.images.get(SKIN_SLOT.stageRight)?.bitmap);
    for (const slot of [SKIN_SLOT.stageHint, SKIN_SLOT.key(0), SKIN_SLOT.stageBottom]) {
      expect(drawn).toContain(SKIN.images.get(slot)?.bitmap);
    }
    expect(second.ops.filter((op) => op.h === layout.columnLineBottom)).toHaveLength(layout.columnLines.length);
  });

  it("rebuilds the background once per change of size, zoom, skin, layout or HitPosition", () => {
    const { factory, layers } = surfaces();
    const twice = (layout: ReturnType<typeof skinLayout>, skin: LoadedSkin = SKIN): void => {
      for (let i = 0; i < 2; i++) {
        const view = { width: layout.width, height: layout.height, judgeY: layout.judgeY };
        drawSkinned(recordingContext().ctx, PROJECTION, layout, skin, T, view, undefined, { background: layers, dpr: 1 });
      }
    };
    const base = skinLayout(SKIN, H, 1);
    twice(base);
    twice(base);
    expect(factory).toHaveBeenCalledTimes(1);
    twice(skinLayout(SKIN, 1200, 1));
    expect(factory).toHaveBeenCalledTimes(2);
    twice(skinLayout(SKIN, 1200, 1.25));
    expect(factory).toHaveBeenCalledTimes(3);
    const raised = { ...SKIN, hitPosition: 400 };
    twice(skinLayout(raised, 1200, 1.25), raised);
    expect(factory).toHaveBeenCalledTimes(4);
    const same = { ...raised };
    twice(skinLayout(same, 1200, 1.25), same);
    expect(factory).toHaveBeenCalledTimes(5);
  });

  it("sizes the background in device pixels and paints it in CSS px", () => {
    const layout = skinLayout(SKIN, H, 1);
    const { made, layers } = surfaces();
    const rec = frame(layout, layers, 2);
    expect(made.map((m) => [m.w, m.h])).toEqual([[Math.round(layout.width * 2), H * 2]]);
    expect(made[0]?.rec.images[0]?.transform).toEqual([2, 0, 0, 2, 0, 0]);
    expect(rec.images[0]).toMatchObject({ sw: Math.round(layout.width * 2), sh: H * 2, dw: layout.width, dh: H });
  });

  it("draws directly, without asking again, when no offscreen surface can be made", () => {
    const layout = skinLayout(SKIN, H, 1);
    const { factory, layers } = surfaces(true);
    const direct = frame(layout, null).all;
    expect(frame(layout, layers).all).toEqual(direct);
    expect(frame(layout, layers).all).toEqual(direct);
    expect(factory).toHaveBeenCalledTimes(1);
  });
});
