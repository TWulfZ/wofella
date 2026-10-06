import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { SKIN_SLOT } from "../skinModel";
import { DEFAULT_SKIN_LOADER_PARAMS, loadSkin } from "./loadSkin";
import { BROKEN_MIME, config7k, fakeCreateImageBitmap, file, skinDto } from "./testSkinDto";

let fake: ReturnType<typeof fakeCreateImageBitmap>;

beforeEach(() => {
  fake = fakeCreateImageBitmap();
  globalThis.createImageBitmap = fake.fn as unknown as typeof createImageBitmap;
});

afterEach(() => {
  Reflect.deleteProperty(globalThis, "createImageBitmap");
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
    });
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
    expect(skin.images.get(SKIN_SLOT.note(0))).toEqual({ bitmap: first, width: 128, height: 64, scale: 2 });
    expect(skin.images.get(SKIN_SLOT.note(6))?.bitmap).toBe(first);
    expect(skin.images.get(SKIN_SLOT.keyDown(3))).toEqual({ bitmap: second, width: 60, height: 200, scale: 1 });
    expect(skin.images.get(SKIN_SLOT.stageHint)?.bitmap).toBe(second);
  });

  it("crops a body taller than the cap to its head end, and only for body slots", async () => {
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
    expect(cropped?.crop).toEqual([0, tall - cap, 50, cap]);
    expect(skin.images.get(SKIN_SLOT.body(0))).toEqual({ bitmap: cropped, width: 50, height: cap, scale: 1 });
    expect(skin.images.get(SKIN_SLOT.stageLeft)?.height).toBe(tall);
    expect(skin.images.get(SKIN_SLOT.stageLeft)?.bitmap).not.toBe(cropped);
  });

  it("leaves a body within the cap whole", async () => {
    await loadSkin(skinDto({ files: [file(50, 300)], images: [{ slot: "body.0", file: 0 }] }));
    expect(fake.bitmaps.map((b) => b.crop)).toEqual([null]);
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

  it("closes every bitmap it created on dispose, once", async () => {
    const tall = DEFAULT_SKIN_LOADER_PARAMS.maxBodyHeightPx * 2;
    const result = await loadSkin(
      skinDto({
        files: [file(10, 10), file(10, tall)],
        images: [
          { slot: "note.0", file: 0 },
          { slot: "body.0", file: 1 },
          { slot: "stage.right", file: 1 },
        ],
      }),
    );
    expect(fake.bitmaps).toHaveLength(3);
    result.dispose();
    result.dispose();
    for (const bitmap of fake.bitmaps) {
      expect(bitmap.close).toHaveBeenCalledTimes(1);
    }
  });
});
