import { describe, expect, it } from "vitest";
import type { AutoplayColumn, AutoplayFrame } from "./autoplay";
import { burstLook, comboScaleY, explosionAlpha } from "./autoplay";
import { COLUMN_COLORS } from "./colors";
import { DEFAULT_PLAYFIELD_THEME as T, draw, drawSkinned } from "./draw";
import { DEFAULT_EFFECT_DRAW_PARAMS as E, type PlayfieldFx } from "./drawEffects";
import { DEFAULT_PLAYFIELD_EFFECTS, type PlayfieldEffects } from "./effects";
import type { NoteSpan, Projection } from "./project";
import { DEFAULT_SKIN_LAYOUT_PARAMS, skinLayout } from "./skinLayout";
import { type LoadedSkin, SKIN_SLOT, type SkinImage, type SkinSlot } from "./skinModel";
import { recordingContext } from "./testCanvas";
import { image, skin7k } from "./testSkin";

// k = 2, unit = 1.25: columns at x 0, 60, 120, 190 (80 wide), 280, 340, 400; the stage is 460 wide, centred on 230;
// the judgement line is at 856 and LightPosition 413 lands at 826.
const H = 960;
const CENTRE = 230;

function projection(spans: NoteSpan[] = []): Projection {
  return {
    notes: [],
    spans,
    beatLines: [],
    handSeparators: [],
    columns: Array.from({ length: 7 }, (_, i) => ({ x: i * 10, w: 10, hand: i < 3 ? "left" : "right" })),
    shade: [],
  };
}

function column(overrides: Partial<AutoplayColumn> = {}): AutoplayColumn {
  return { keyDown: false, stageLight: 0, explosions: [], hold: null, ...overrides };
}

function frame(overrides: Partial<AutoplayFrame> = {}, columns: Record<number, Partial<AutoplayColumn>> = {}): AutoplayFrame {
  return {
    combo: 0,
    sinceJudgementMs: null,
    columns: Array.from({ length: 7 }, (_, i) => column(columns[i])),
    ...overrides,
  };
}

function fx(flags: Partial<PlayfieldEffects>, f: AutoplayFrame | null = frame()): PlayfieldFx {
  return { flags: { ...DEFAULT_PLAYFIELD_EFFECTS, ...flags }, frame: f };
}

function render(skin: LoadedSkin, spans: NoteSpan[], effects?: PlayfieldFx, zoom = 1) {
  const layout = skinLayout(skin, H, zoom);
  const rec = recordingContext();
  const view = { width: layout.width, height: H, judgeY: layout.judgeY };
  drawSkinned(rec.ctx, projection(spans), layout, skin, T, view, undefined, undefined, effects);
  return { ...rec, layout };
}

const withImages = (images: [SkinSlot, SkinImage][], overrides: Partial<LoadedSkin> = {}) => skin7k(overrides, images);
const imagesOf = (all: ReturnType<typeof render>["images"], img: SkinImage) => all.filter((op) => op.image === img.bitmap);

describe("percy switch", () => {
  const note = image(100, 50);
  const percyBody = image(138, 4096, 1, 40_000);
  const percySkin = withImages([
    [SKIN_SLOT.note(0), note],
    [SKIN_SLOT.body(0), percyBody],
    [SKIN_SLOT.tail(0), image(256, 1)],
  ]);
  const LN: NoteSpan = { col: 0, headY: 800, tailY: 300 };
  const gap = T.noteGapPx;

  it("draws exactly as before with percy on, whatever the other toggles say when nothing happens", () => {
    const before = render(percySkin, [LN]).all;
    expect(render(percySkin, [LN], fx({})).all).toEqual(before);
    expect(render(percySkin, [LN], fx({ judgements: true, combo: true, keyPress: true, lighting: true })).all).toEqual(
      before,
    );
  });

  it("with percy off, draws a plain body in the column colour, an opaque tail cap, and the skin's head", () => {
    const { all, images } = render(percySkin, [LN], fx({ percy: false }));
    expect(imagesOf(images, percyBody)).toEqual([]);
    const capH = DEFAULT_SKIN_LAYOUT_PARAMS.fallbackTailHeightPx;
    const body = { type: "fill", x: gap, y: 300, w: 60 - 2 * gap, h: 500, fill: COLUMN_COLORS.outer, alpha: T.lnBodyAlpha };
    const cap = { type: "fill", x: gap, y: 300 - capH, w: 60 - 2 * gap, h: capH, fill: COLUMN_COLORS.outer, alpha: 1 };
    const head: unknown = expect.objectContaining({ type: "image", image: note.bitmap, dy: 758, dh: 42 });
    const drawn = all.filter((op) => (op.type === "fill" && op.fill === COLUMN_COLORS.outer) || op.type === "image");
    expect(drawn).toEqual([body, cap, head]);
  });

  it("leaves a tiled (non-percy) body alone with percy off", () => {
    const tiled = withImages([
      [SKIN_SLOT.note(0), note],
      [SKIN_SLOT.body(0), image(30, 30)],
    ]);
    expect(render(tiled, [LN], fx({ percy: false })).all).toEqual(render(tiled, [LN]).all);
  });
});

describe("key presses", () => {
  const up = image(60, 200, 2);
  const down = image(60, 240, 2);
  const otherUp = image(60, 200, 2);
  const skin = withImages([
    [SKIN_SLOT.key(0), up],
    [SKIN_SLOT.keyDown(0), down],
    [SKIN_SLOT.key(1), otherUp],
  ]);
  const pressed = frame({}, { 0: { keyDown: true }, 1: { keyDown: true } });

  it("swaps a held column's key for its down image, bottom-anchored at its own height", () => {
    const { images } = render(skin, [], fx({ keyPress: true }, pressed));
    expect(imagesOf(images, up)).toEqual([]);
    expect(imagesOf(images, down)).toEqual([expect.objectContaining({ dx: 0, dy: H - 240, dw: 60, dh: 240, alpha: 1 })]);
    expect(imagesOf(images, otherUp)).toHaveLength(1);
  });

  it("keeps the up image while the toggle is off", () => {
    const { images } = render(skin, [], fx({ keyPress: false }, pressed));
    expect(imagesOf(images, down)).toEqual([]);
    expect(imagesOf(images, up)).toHaveLength(1);
  });
});

describe("lighting", () => {
  const light = image(20, 100);
  const tinted = image(20, 100);
  const hint = image(100, 20);
  const note = image(100, 50);

  it("draws the column's tinted stage light under the notes, its bottom on LightPosition, shrinking as it fades", () => {
    const skin = withImages(
      [
        [SKIN_SLOT.stageLight, light],
        [SKIN_SLOT.stageHint, hint],
        [SKIN_SLOT.note(1), note],
      ],
      { stageLights: new Map([[1, tinted]]) },
    );
    const f = frame({}, { 1: { stageLight: 0.5 }, 2: { stageLight: 1 } });
    const { all, images } = render(skin, [{ col: 1, headY: 500, tailY: null }], fx({ lighting: true }, f));
    expect(imagesOf(images, tinted)).toEqual([
      expect.objectContaining({ dx: 60, dw: 60, dh: 62.5, dy: 826 - 62.5, alpha: 0.5 }),
    ]);
    expect(imagesOf(images, light)).toEqual([expect.objectContaining({ dx: 120, dw: 60, dh: 125, dy: 701, alpha: 1 })]);
    const index = (img: SkinImage) => all.findIndex((op) => op.type === "image" && op.image === img.bitmap);
    expect(index(hint)).toBeLessThan(index(tinted));
    expect(index(tinted)).toBeLessThan(index(note));

    expect(imagesOf(render(skin, [], fx({ lighting: false }, f)).images, tinted)).toEqual([]);
  });

  const n0 = image(64, 64);
  const n1 = image(64, 64);

  it("bursts LightingN additively on the judgement line, scaled by the column width over 30", () => {
    const skin = withImages([
      [SKIN_SLOT.lightingN(0), n0],
      [SKIN_SLOT.lightingN(1), n1],
    ]);
    const { images } = render(skin, [], fx({ lighting: true }, frame({}, { 3: { explosions: [40, 100] } })));
    const size = 64 * 1.25 * (40 / 30);
    const at = { dx: CENTRE - size / 2, dy: 856 - size / 2, composite: "lighter" };
    expect(imagesOf(images, n0)).toEqual([expect.objectContaining({ ...at, alpha: explosionAlpha(40) })]);
    expect(imagesOf(images, n1)).toEqual([expect.objectContaining({ ...at, alpha: explosionAlpha(100) })]);
    expect(imagesOf(images, n0)[0]?.dw).toBeCloseTo(size);
  });

  it("uses LightingNWidth when set, and no scale before Version 2.5", () => {
    const base = withImages([[SKIN_SLOT.lightingN(0), n0]]);
    const f = fx({ lighting: true }, frame({}, { 0: { explosions: [40] } }));
    const widths = { ...base.effects, lightingNWidth: [60, 0, 0, 0, 0, 0, 0] };
    expect(imagesOf(render({ ...base, effects: widths }, [], f).images, n0)[0]?.dw).toBeCloseTo(64 * 1.25 * 2);
    expect(imagesOf(render({ ...base, version: 2.4 }, [], f).images, n0)[0]?.dw).toBeCloseTo(64 * 1.25);
  });

  it("loops LightingL over a held LN at the hold's alpha", () => {
    const frames = [image(40, 40), image(40, 40), image(40, 40)];
    const skin = withImages(frames.map((img, i): [SkinSlot, SkinImage] => [SKIN_SLOT.lightingL(i), img]));
    const { images } = render(skin, [], fx({ lighting: true }, frame({}, { 0: { hold: { sinceStartMs: 100, alpha: 0.75 } } })));
    expect(images.filter((op) => frames.some((img) => img.bitmap === op.image))).toEqual([
      expect.objectContaining({ image: frames[1]?.bitmap, dx: 30 - 25, dy: 856 - 25, dw: 50, dh: 50, alpha: 0.75, composite: "lighter" }),
    ]);
  });
});

describe("judgement bursts", () => {
  const max = image(100, 40);
  const great = image(90, 40);
  const playing = frame({ sinceJudgementMs: 100 });

  it("shows MAX from mania-hit300g at ScorePosition, centred on the stage, with stable's scale", () => {
    const { images } = render(
      withImages([
        [SKIN_SLOT.hit("300g"), max],
        [SKIN_SLOT.hit("300"), great],
      ]),
      [],
      fx({ judgements: true }, playing),
    );
    const look = burstLook(100);
    expect(look).toEqual({ alpha: 1, scale: 0.7 });
    expect(images).toEqual([
      expect.objectContaining({ image: max.bitmap, dx: CENTRE - 43.75, dy: 600 - 17.5, dw: 87.5, dh: 35, alpha: 1 }),
    ]);
  });

  it("falls back to the next available burst, then to text", () => {
    const fifty = image(80, 40);
    expect(render(withImages([[SKIN_SLOT.hit("50"), fifty]]), [], fx({ judgements: true }, playing)).images).toEqual([
      expect.objectContaining({ image: fifty.bitmap }),
    ]);
    const { texts, images } = render(skin7k(), [], fx({ judgements: true }, playing));
    expect(images).toEqual([]);
    expect(texts).toEqual([
      expect.objectContaining({
        text: E.maxText,
        x: 0,
        y: 0,
        align: "center",
        baseline: "middle",
        fill: T.maxText,
        alpha: 1,
        transform: [0.7, 0, 0, 0.7, CENTRE, 600],
      }),
    ]);
  });

  it("follows the skin's ScorePosition", () => {
    const skin = withImages([[SKIN_SLOT.hit("300g"), max]]);
    const moved = { ...skin, effects: { ...skin.effects, scorePosition: 250 } };
    expect(render(moved, [], fx({ judgements: true }, playing)).images[0]?.dy).toBe(500 - 17.5);
  });

  it.each([
    ["the toggle is off", fx({ judgements: false }, playing)],
    ["nothing was judged yet", fx({ judgements: true }, frame())],
    ["the burst is over", fx({ judgements: true }, frame({ sinceJudgementMs: 220 }))],
  ])("draws no burst when %s", (_, effects) => {
    const rec = render(withImages([[SKIN_SLOT.hit("300g"), max]]), [], effects);
    expect(rec.images).toEqual([]);
    expect(rec.texts).toEqual([]);
  });
});

describe("stage zoom", () => {
  // Zoom widens the stage; LightPosition, ScorePosition and ComboPosition stay put, like the judgement line.
  it.each([0.75, 2])("keeps the light bottom, the burst and the combo at their zoom-1 heights at zoom %s", (zoom) => {
    const light = image(20, 100);
    const burst = image(100, 40);
    const digit = image(22, 30);
    const skin = withImages([
      [SKIN_SLOT.stageLight, light],
      [SKIN_SLOT.hit("300g"), burst],
      ...Array.from({ length: 10 }, (_, d): [SkinSlot, SkinImage] => [SKIN_SLOT.comboDigit(d), digit]),
    ]);
    const f = frame({ combo: 3, sinceJudgementMs: 100 }, { 1: { stageLight: 1 } });
    const { images, layout } = render(skin, [], fx({ lighting: true, judgements: true, combo: true }, f), zoom);
    expect(layout.judgeY).toBe(856);
    const lit = imagesOf(images, light)[0];
    expect((lit?.dy ?? 0) + (lit?.dh ?? 0)).toBeCloseTo(826);
    const shown = imagesOf(images, burst)[0];
    expect((shown?.dy ?? 0) + (shown?.dh ?? 0) / 2).toBeCloseTo(600);
    const glyph = imagesOf(images, digit)[0];
    expect((glyph?.dy ?? 0) + (glyph?.dh ?? 0) / 2).toBeCloseTo(222);
  });

  it("keeps the text stand-ins at their zoom-1 heights", () => {
    const f = frame({ combo: 12, sinceJudgementMs: 100 });
    const { texts } = render(skin7k(), [], fx({ judgements: true, combo: true }, f), 2);
    expect(texts.map((t) => t.transform[5])).toEqual([600, 222]);
  });
});

describe("combo", () => {
  const one = image(20, 30);
  const two = image(24, 30);
  const digits = (): [SkinSlot, SkinImage][] =>
    Array.from({ length: 10 }, (_, d): [SkinSlot, SkinImage] => [
      SKIN_SLOT.comboDigit(d),
      d === 1 ? one : d === 2 ? two : image(22, 30),
    ]);

  it("lays the skin's digits out at ComboPosition, centred, overlapping by ComboOverlap", () => {
    const base = withImages(digits());
    const skin = { ...base, effects: { ...base.effects, comboOverlap: 2 } };
    const { images } = render(skin, [], fx({ combo: true }, frame({ combo: 12, sinceJudgementMs: 300 })));
    expect(images).toEqual([
      expect.objectContaining({ image: one.bitmap, dx: 203.75, dw: 25, dy: 222 - 18.75, dh: 37.5 }),
      expect.objectContaining({ image: two.bitmap, dx: 226.25, dw: 30, dy: 222 - 18.75, dh: 37.5 }),
    ]);
  });

  it("pops the digits taller right after a judgement, about their centre", () => {
    const { images } = render(withImages(digits()), [], fx({ combo: true }, frame({ combo: 2, sinceJudgementMs: 0 })));
    const h = 37.5 * comboScaleY(0);
    expect(images[0]?.dh).toBeCloseTo(h);
    expect(images[0]?.dy).toBeCloseTo(222 - h / 2);
  });

  it("writes the number as text when the skin lacks a digit it needs", () => {
    const { texts, images } = render(
      withImages([[SKIN_SLOT.comboDigit(1), one]]),
      [],
      fx({ combo: true }, frame({ combo: 12, sinceJudgementMs: 300 })),
    );
    expect(images).toEqual([]);
    expect(texts).toEqual([
      expect.objectContaining({ text: "12", fill: T.comboText, transform: [1, 0, 0, 1, CENTRE, 222] }),
    ]);
  });

  it("shows nothing at combo 0 or with the toggle off", () => {
    expect(render(withImages(digits()), [], fx({ combo: true }, frame({ combo: 0 }))).images).toEqual([]);
    expect(render(withImages(digits()), [], fx({ combo: false }, frame({ combo: 5, sinceJudgementMs: 50 }))).images).toEqual(
      [],
    );
  });

  it("draws bursts and combo above notes, keys and the shade", () => {
    const note = image(100, 50);
    const key = image(60, 200, 2);
    const max = image(100, 40);
    const skin = withImages([...digits(), [SKIN_SLOT.note(0), note], [SKIN_SLOT.key(0), key], [SKIN_SLOT.hit("300g"), max]]);
    const { all } = render(
      skin,
      [{ col: 0, headY: 700, tailY: null }],
      fx({ combo: true, judgements: true }, frame({ combo: 1, sinceJudgementMs: 100 })),
    );
    const index = (img: SkinImage) => all.findIndex((op) => op.type === "image" && op.image === img.bitmap);
    expect(index(note)).toBeLessThan(index(key));
    expect(index(key)).toBeLessThan(index(max));
    expect(index(max)).toBeLessThan(index(one));
  });
});

describe("procedural effects", () => {
  const VIEW = { width: 300, height: 600, judgeY: 500 };
  const PROJECTION: Projection = { ...projection(), columns: [{ x: 0, w: 100, hand: "left" }] };

  it("writes MAX and the combo as text at stable's default positions, ignoring keys and lighting", () => {
    const rec = recordingContext();
    const f = frame({ combo: 7, sinceJudgementMs: 100 }, { 0: { keyDown: true, stageLight: 1, explosions: [40] } });
    draw(rec.ctx, PROJECTION, T, VIEW, fx({ judgements: true, combo: true, keyPress: true, lighting: true }, f));
    const k = 600 / 480;
    expect(rec.texts).toEqual([
      expect.objectContaining({ text: E.maxText, transform: [0.7, 0, 0, 0.7, 150, 300 * k], fill: T.maxText }),
      expect.objectContaining({ text: "7", transform: [1, 0, 0, comboScaleY(100), 150, 111 * k], fill: T.comboText }),
    ]);
    expect(rec.texts[0]?.font).toBe(`bold ${E.judgementTextUnits * k}px ${E.textFamily}`);
    expect(rec.images).toEqual([]);
  });

  it("draws exactly as before without effects", () => {
    const plain = recordingContext();
    draw(plain.ctx, PROJECTION, T, VIEW);
    const off = recordingContext();
    draw(off.ctx, PROJECTION, T, VIEW, fx({}, frame({ combo: 3, sinceJudgementMs: 10 })));
    expect(off.all).toEqual(plain.all);
  });
});
