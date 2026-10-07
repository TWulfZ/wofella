import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SKIN_SLOT } from "../skinModel";
import { fakeOffscreenCanvas } from "../testCanvas";
import { DEFAULT_SKIN_LOADER_PARAMS, loadSkin } from "./loadSkin";
import { BROKEN_MIME, config7k, fakeCreateImageBitmap, file, skinDto, skinEffects } from "./testSkinDto";

let fake: ReturnType<typeof fakeCreateImageBitmap>;
let surfaces: ReturnType<typeof fakeOffscreenCanvas>;

beforeEach(() => {
  fake = fakeCreateImageBitmap();
  globalThis.createImageBitmap = fake.fn as unknown as typeof createImageBitmap;
  surfaces = fakeOffscreenCanvas();
  vi.stubGlobal("OffscreenCanvas", surfaces.FakeOffscreenCanvas);
});

afterEach(() => {
  Reflect.deleteProperty(globalThis, "createImageBitmap");
  vi.unstubAllGlobals();
});

describe("loadSkin", () => {
  it("decodes without EXIF rotation so bitmaps match the header sizes the renderer uses", async () => {
    await loadSkin(
      skinDto({
        files: [file(128, 64), file(50, 9000)],
        images: [
          { slot: "note.0", file: 0 },
          { slot: "body.0", file: 1 },
        ],
      }),
    );
    expect(fake.fn).toHaveBeenCalledTimes(2);
    for (const call of fake.fn.mock.calls) {
      expect(call.at(-1)).toEqual({ imageOrientation: "none" });
    }
  });

  it("maps the config, version and colours onto the renderer's skin", async () => {
    const { skin, failedSlots } = await loadSkin(
      skinDto({
        version: 2.5,
        config: config7k({
          columnSpacing: [0, 0, 5, 5, 0, 0],
          noteBodyStyle: "stretch",
          keysUnderNotes: true,
          widthForNoteHeightScale: null,
          colours: {
            column: [[10, 20, 30, 40], null],
            columnLine: [255, 255, 255, 128],
            judgementLine: null,
            barline: null,
            hold: null,
          },
        }),
      }),
    );
    expect(failedSlots).toEqual([]);
    expect(skin).toEqual({
      version: 2.5,
      columnWidth: [40, 42, 42, 42, 42, 42, 42],
      columnSpacing: [0, 0, 5, 5, 0, 0],
      columnLineWidth: [2, 2, 2, 2, 2, 2, 2, 2],
      hitPosition: 428,
      lightPosition: 413,
      widthForNoteHeightScale: null,
      noteBodyStyle: "Stretch",
      judgementLine: true,
      keysUnderNotes: true,
      colours: {
        column: [{ r: 10, g: 20, b: 30, a: 40 }, null],
        columnLine: { r: 255, g: 255, b: 255, a: 128 },
        judgementLine: null,
      },
      images: new Map(),
      lnTails: new Map(),
      effects: {
        scorePosition: 300,
        comboPosition: 111,
        lightingNWidth: [0, 0, 0, 0, 0, 0, 0],
        lightingLWidth: [0, 0, 0, 0, 0, 0, 0],
        comboOverlap: 0,
      },
      stageLights: new Map(),
    });
  });

  it("maps the effect placement, and reads null numbers and short per-column lists as lazer's defaults", async () => {
    const effects = {
      scorePosition: 250,
      comboPosition: 140,
      lightingNWidth: [40, null, 40, 40, 40, 40, 40],
      lightingLWidth: [50, 50, 50, 50, 50, 50, 50],
      lightColours: [],
      comboOverlap: -2,
    };
    const { skin } = await loadSkin(skinDto({ effects }));
    expect(skin.effects).toEqual({
      scorePosition: 250,
      comboPosition: 140,
      lightingNWidth: [40, 0, 40, 40, 40, 40, 40],
      lightingLWidth: [50, 50, 50, 50, 50, 50, 50],
      comboOverlap: -2,
    });

    const p = DEFAULT_SKIN_LOADER_PARAMS;
    const nulls = { ...effects, scorePosition: null, comboPosition: null, comboOverlap: null };
    const loaded = (await loadSkin(skinDto({ effects: nulls }))).skin.effects;
    expect(loaded.scorePosition).toBe(p.defaultScorePosition);
    expect(loaded.comboPosition).toBe(p.defaultComboPosition);
    expect(loaded.comboOverlap).toBe(0);
    const short = skinDto({ effects: skinEffects(7, { lightingNWidth: [], lightingLWidth: [30] }) });
    expect((await loadSkin(short)).skin.effects.lightingNWidth).toEqual([0, 0, 0, 0, 0, 0, 0]);
    expect((await loadSkin(short)).skin.effects.lightingLWidth).toEqual([30, 0, 0, 0, 0, 0, 0]);
    expect((await loadSkin(skinDto({ config: config7k({ lightPosition: null }) }))).skin.lightPosition).toBe(
      p.defaultLightPosition,
    );
  });

  it("keeps the playback effect slots: hit bursts, combo digits and lighting frames", async () => {
    const slots = ["hit.300g", "hit.0", "combo.0", "combo.9", "lighting.n.0", "lighting.n.11", "lighting.l.0"];
    const { skin, failedSlots } = await loadSkin(
      skinDto({ files: [file(10, 10)], images: slots.map((slot) => ({ slot, file: 0 })) }),
    );
    expect([...skin.images.keys()]).toEqual(slots);
    expect(failedSlots).toEqual([]);
    expect(skin.images.get(SKIN_SLOT.lightingN(11))).toBeDefined();
  });

  it("fills numbers the wire left null with skin.ini's defaults", async () => {
    const p = DEFAULT_SKIN_LOADER_PARAMS;
    const { skin } = await loadSkin(
      skinDto({
        version: null,
        config: config7k({
          columnWidth: [null, 42, 42, 42, 42, 42, 42],
          columnSpacing: [null, 0, 0, 0, 0, 0],
          columnLineWidth: [null, 2, 2, 2, 2, 2, 2, 2],
          hitPosition: null,
        }),
      }),
    );
    expect(skin.version).toBe(p.defaultVersion);
    expect(skin.columnWidth[0]).toBe(p.defaultColumnWidth);
    expect(skin.columnSpacing[0]).toBe(p.defaultColumnSpacing);
    expect(skin.columnLineWidth[0]).toBe(p.defaultColumnLineWidth);
    expect(skin.hitPosition).toBe(p.defaultHitPosition);
  });

  it("decodes each file once into a bitmap shared by its slots, keeping its pixel size and @2x scale", async () => {
    const { skin } = await loadSkin(
      skinDto({
        files: [file(128, 64, { scale: 2 }), file(60, 200, { mime: "image/jpeg" })],
        images: [
          { slot: "note.0", file: 0 },
          { slot: "note.6", file: 0 },
          { slot: "key.3.down", file: 1 },
          { slot: "stage.hint", file: 1 },
        ],
      }),
    );
    expect(fake.fn).toHaveBeenCalledTimes(2);
    const [png, jpeg] = fake.fn.mock.calls.map(([blob]) => blob);
    expect(png?.type).toBe("image/png");
    expect(jpeg?.type).toBe("image/jpeg");
    expect(await png?.arrayBuffer().then((b) => [...new Uint8Array(b)])).toEqual([
      0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    ]);
    const [first, second] = fake.bitmaps;
    expect([...skin.images.keys()]).toEqual(["note.0", "note.6", "key.3.down", "stage.hint"]);
    expect(skin.images.get(SKIN_SLOT.note(0))).toEqual({ bitmap: first, width: 128, height: 64, scale: 2, sourceHeight: 64 });
    expect(skin.images.get(SKIN_SLOT.note(6))?.bitmap).toBe(first);
    expect(skin.images.get(SKIN_SLOT.keyDown(3))).toEqual({
      bitmap: second,
      width: 60,
      height: 200,
      scale: 1,
      sourceHeight: 200,
    });
    expect(skin.images.get(SKIN_SLOT.stageHint)?.bitmap).toBe(second);
  });

  it("crops a body taller than the cap to its top rows, keeps its source height, and only for body slots", async () => {
    const cap = DEFAULT_SKIN_LOADER_PARAMS.maxBodyHeightPx;
    const tall = cap + 1000;
    const { skin } = await loadSkin(
      skinDto({
        files: [file(50, tall)],
        images: [
          { slot: "body.0", file: 0 },
          { slot: "stage.left", file: 0 },
        ],
      }),
    );
    const cropped = fake.bitmaps.find((b) => b.crop !== null);
    expect(cropped?.crop).toEqual([0, 0, 50, cap]);
    expect(skin.images.get(SKIN_SLOT.body(0))).toEqual({
      bitmap: cropped,
      width: 50,
      height: cap,
      scale: 1,
      sourceHeight: tall,
    });
    expect(skin.images.get(SKIN_SLOT.stageLeft)?.sourceHeight).toBe(tall);
    expect(skin.images.get(SKIN_SLOT.stageLeft)?.height).toBe(tall);
    expect(skin.images.get(SKIN_SLOT.stageLeft)?.bitmap).not.toBe(cropped);
  });

  it("flips each column's LN tail once at load, following the tail, head, note chain", async () => {
    const { skin } = await loadSkin(
      skinDto({
        files: [file(100, 50), file(100, 20), file(90, 30)],
        images: [
          { slot: "note.0", file: 0 },
          { slot: "note.0.tail", file: 1 },
          { slot: "note.1", file: 0 },
          { slot: "note.2", file: 0 },
          { slot: "note.3.head", file: 2 },
        ],
      }),
    );
    const [note, tail, head] = fake.bitmaps;
    expect(surfaces.instances.map((c) => [c.width, c.height])).toEqual([
      [100, 20],
      [100, 50],
      [90, 30],
    ]);
    const [flippedTail, flippedNote, flippedHead] = surfaces.instances;
    for (const [canvas, source, h] of [
      [flippedTail, tail, 20],
      [flippedNote, note, 50],
      [flippedHead, head, 30],
    ] as const) {
      expect(canvas?.rec.images).toEqual([
        expect.objectContaining({ image: source, sy: 0, sh: h, dy: 0, dh: h, transform: [1, 0, 0, -1, 0, h] }),
      ]);
    }
    expect([...skin.lnTails.keys()]).toEqual([0, 1, 2, 3]);
    expect(skin.lnTails.get(0)).toEqual({ bitmap: flippedTail, width: 100, height: 20, scale: 1, sourceHeight: 20 });
    expect(skin.lnTails.get(1)?.bitmap).toBe(flippedNote);
    expect(skin.lnTails.get(2)?.bitmap).toBe(flippedNote);
    expect(skin.lnTails.get(3)?.bitmap).toBe(flippedHead);
    expect(skin.images.get(SKIN_SLOT.tail(0))?.bitmap).toBe(tail);
  });

  it("leaves a column's tail out when no offscreen surface can be made", async () => {
    vi.stubGlobal("OffscreenCanvas", undefined);
    const { skin } = await loadSkin(skinDto({ files: [file(100, 50)], images: [{ slot: "note.0", file: 0 }] }));
    expect(skin.lnTails.size).toBe(0);
    expect(skin.images.get(SKIN_SLOT.note(0))).toBeDefined();
  });

  it("leaves a body within the cap whole", async () => {
    const { skin } = await loadSkin(skinDto({ files: [file(50, 300)], images: [{ slot: "body.0", file: 0 }] }));
    expect(fake.bitmaps.map((b) => b.crop)).toEqual([null]);
    expect(skin.images.get(SKIN_SLOT.body(0))).toMatchObject({ height: 300, sourceHeight: 300 });
  });

  it("drops only the slots of a file that fails to decode, and reports them", async () => {
    const { skin, failedSlots } = await loadSkin(
      skinDto({
        files: [file(10, 10, { mime: BROKEN_MIME }), file(10, 10), file(10, 10, { base64: "not base64!" })],
        images: [
          { slot: "note.0", file: 0 },
          { slot: "note.0.head", file: 0 },
          { slot: "note.1", file: 1 },
          { slot: "key.0", file: 2 },
        ],
      }),
    );
    expect([...skin.images.keys()]).toEqual(["note.1"]);
    expect(failedSlots).toEqual(["note.0", "note.0.head", "key.0"]);
  });

  it("ignores slot ids the renderer does not know and file indices out of range", async () => {
    const { skin, failedSlots } = await loadSkin(
      skinDto({
        files: [file(10, 10)],
        images: [
          { slot: "lighting.0", file: 0 },
          { slot: "note.x", file: 0 },
          { slot: "note.2", file: 7 },
          { slot: "stage.light", file: 0 },
        ],
      }),
    );
    expect([...skin.images.keys()]).toEqual(["stage.light"]);
    expect(failedSlots).toEqual(["note.2"]);
  });

  it("tints stage.light per column with ColourLight, the wiki's default where unset, painting each colour once", async () => {
    const own: [number, number, number, number] = [10, 20, 30, 128];
    const { skin } = await loadSkin(
      skinDto({
        files: [file(20, 100)],
        images: [{ slot: "stage.light", file: 0 }],
        effects: {
          scorePosition: 300,
          comboPosition: 111,
          lightingNWidth: [],
          lightingLWidth: [],
          lightColours: [own, null, null, null, null, null, own],
          comboOverlap: 0,
        },
      }),
    );
    const [light] = fake.bitmaps;
    expect(surfaces.instances.map((c) => [c.width, c.height])).toEqual([
      [20, 100],
      [20, 100],
    ]);
    const [ownTint, defaultTint] = surfaces.instances;
    const d = DEFAULT_SKIN_LOADER_PARAMS.defaultLightColour;
    for (const [canvas, rgb, alpha] of [
      [ownTint, "rgb(10, 20, 30)", 128 / 255],
      [defaultTint, `rgb(${d.r}, ${d.g}, ${d.b})`, 1],
    ] as const) {
      expect(canvas?.rec.all).toEqual([
        expect.objectContaining({ type: "image", image: light, dx: 0, dy: 0, dw: 20, dh: 100, alpha: 1 }),
        expect.objectContaining({ type: "fill", x: 0, y: 0, w: 20, h: 100, fill: rgb, composite: "multiply" }),
        expect.objectContaining({ type: "image", image: light, alpha, composite: "destination-in" }),
      ]);
      expect(canvas?.rec.ctx.globalCompositeOperation).toBe("source-over");
    }
    expect([...skin.stageLights.keys()]).toEqual([0, 1, 2, 3, 4, 5, 6]);
    expect(skin.stageLights.get(0)).toEqual({ bitmap: ownTint, width: 20, height: 100, scale: 1, sourceHeight: 100 });
    expect(skin.stageLights.get(6)?.bitmap).toBe(ownTint);
    expect(skin.stageLights.get(3)?.bitmap).toBe(defaultTint);
  });

  it("leaves stage lights untinted when no offscreen surface can be made", async () => {
    vi.stubGlobal("OffscreenCanvas", undefined);
    const { skin } = await loadSkin(skinDto({ files: [file(20, 100)], images: [{ slot: "stage.light", file: 0 }] }));
    expect(skin.stageLights.size).toBe(0);
    expect(skin.images.get(SKIN_SLOT.stageLight)).toBeDefined();
  });

  it("closes every bitmap it created on dispose, once", async () => {
    const tall = DEFAULT_SKIN_LOADER_PARAMS.maxBodyHeightPx * 2;
    const result = await loadSkin(
      skinDto({
        files: [file(10, 10), file(10, tall)],
        images: [
          { slot: "note.0", file: 0 },
          { slot: "body.0", file: 1 },
          { slot: "stage.right", file: 1 },
          { slot: "stage.light", file: 0 },
        ],
      }),
    );
    expect(fake.bitmaps).toHaveLength(3);
    expect(surfaces.instances).toHaveLength(2);
    result.dispose();
    result.dispose();
    for (const bitmap of fake.bitmaps) {
      expect(bitmap.close).toHaveBeenCalledTimes(1);
    }
    expect(surfaces.instances.map((c) => [c.width, c.height])).toEqual([
      [0, 0],
      [0, 0],
    ]);
  });
});
