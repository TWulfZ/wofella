import { describe, expect, it } from "vitest";
import { DEFAULT_PLAYFIELD_PARAMS } from "./project";
import { DEFAULT_SKIN_LAYOUT_PARAMS as P, doubledAlpha, noteImage, opaqueAlpha, skinLayout } from "./skinLayout";
import { SKIN_SLOT } from "./skinModel";
import { image, skin7k } from "./testSkin";

// H = 960 makes k = 2 at zoom 1, and one lazer unit (768-high space) 1.25 px.
const H = 960;

describe("skinLayout geometry", () => {
  it("scales ColumnWidth and ColumnSpacing by k = H/480·zoom and puts the judgement line at HitPosition", () => {
    const layout = skinLayout(skin7k(), H, 1);
    expect(layout.k).toBe(2);
    expect(layout.columns.map((c) => [c.x, c.w])).toEqual([
      [0, 60],
      [60, 60],
      [120, 60],
      [190, 80],
      [280, 60],
      [340, 60],
      [400, 60],
    ]);
    expect(layout.stageX).toBe(0);
    expect(layout.stageWidth).toBe(460);
    expect(layout.width).toBe(460);
    expect(layout.judgeY).toBe(856);
  });

  it("widens the stage with zoom but keeps the judgement line where HitPosition puts it", () => {
    const layout = skinLayout(skin7k(), H, 1.5);
    expect(layout.k).toBe(3);
    expect(layout.stageWidth).toBe(690);
    expect(layout.judgeY).toBe(856);
    expect(skinLayout(skin7k({ hitPosition: 100 }), H, 1).judgeY).toBe(480);
  });

  it("puts stage-left and stage-right outside the columns, stretched to the full height", () => {
    const layout = skinLayout(
      skin7k({}, [
        [SKIN_SLOT.stageLeft, image(40, 768, 2)],
        [SKIN_SLOT.stageRight, image(16, 10)],
      ]),
      H,
      1,
    );
    expect(layout.stageLeft).toEqual({ x: 0, y: 0, w: 25, h: H });
    expect(layout.stageX).toBe(25);
    expect(layout.columns[0]?.x).toBe(25);
    expect(layout.stageRight).toEqual({ x: 485, y: 0, w: 20, h: H });
    expect(layout.width).toBe(505);
  });

  it("draws column lines 0.74 wide in lazer units down to the judgement line, right lines from Version 2.4", () => {
    const w = 2 * P.columnLineScaleX * 1.25;
    const modern = skinLayout(skin7k(), H, 1);
    expect(modern.columnLineBottom).toBe(856);
    expect(modern.columnLines).toHaveLength(14);
    expect(modern.columnLines.slice(0, 2)).toEqual([
      { x: 0, w },
      { x: 60, w },
    ]);
    expect(modern.columnLines.at(-1)?.x).toBeCloseTo(460 + P.lastColumnLineOffsetUnits * 1.25);

    const old = skinLayout(skin7k({ version: 2.0, columnLineWidth: [0, 2, 2, 2, 2, 2, 2, 3] }), H, 1);
    expect(old.columnLines.map((l) => l.x).slice(0, -1)).toEqual([60, 120, 190, 280, 340, 400]);
    expect(old.columnLines.at(-1)).toEqual({
      x: 460 + P.lastColumnLineOffsetUnits * 1.25,
      w: 3 * P.columnLineScaleX * 1.25,
    });
  });

  it("sizes a note from its texture's display size and WidthForNoteHeightScale", () => {
    const note = image(256, 150, 2);
    const withScale = skinLayout(skin7k({}, [[SKIN_SLOT.note(0), note]]), H, 1);
    expect(withScale.columns[0]?.noteH).toBeCloseTo((75 * 42 * 2) / 128);
    const narrowest = skinLayout(skin7k({ widthForNoteHeightScale: null }, [[SKIN_SLOT.note(0), note]]), H, 1);
    expect(narrowest.columns[0]?.noteH).toBeCloseTo((75 * 30 * 2) / 128);
    // An unparsable skin.ini entry reads as 0 (lazer's list rule); 0 would hide every note.
    const zero = skinLayout(skin7k({ widthForNoteHeightScale: 0 }, [[SKIN_SLOT.note(0), note]]), H, 1);
    expect(zero.columns[0]?.noteH).toBeCloseTo((75 * 30 * 2) / 128);
  });

  it("sizes LN heads and tails from the image each one falls back to, and procedural slots from the params", () => {
    const layout = skinLayout(
      skin7k({}, [
        [SKIN_SLOT.note(0), image(100, 50)],
        [SKIN_SLOT.head(0), image(100, 20)],
        [SKIN_SLOT.note(1), image(100, 10)],
      ]),
      H,
      1,
    );
    const per = 84 / 100;
    expect(layout.columns[0]?.headH).toBeCloseTo(20 * per);
    expect(layout.columns[0]?.tailH).toBeCloseTo(20 * per);
    expect(layout.columns[1]?.headH).toBeCloseTo(10 * per);
    expect(layout.columns[1]?.tailH).toBeCloseTo(10 * per);
    expect(layout.columns[2]?.noteH).toBe(DEFAULT_PLAYFIELD_PARAMS.noteHeightPx);
    expect(layout.columns[2]?.tailH).toBe(DEFAULT_PLAYFIELD_PARAMS.lnTailHeightPx);
  });

  it("anchors keys to the bottom at their texture display height times k", () => {
    const layout = skinLayout(skin7k({}, [[SKIN_SLOT.key(3), image(60, 200, 2)]]), H, 1);
    expect(layout.columns[3]?.key).toEqual({ x: 190, y: H - 200, w: 80, h: 200 });
    expect(layout.columns[0]?.key).toBeNull();
  });

  it("centres the hit target on the judgement line across the stage and puts stage-bottom at the bottom", () => {
    const layout = skinLayout(
      skin7k({}, [
        [SKIN_SLOT.stageHint, image(100, 20)],
        [SKIN_SLOT.stageBottom, image(100, 30, 2)],
      ]),
      H,
      1,
    );
    const hintH = 20 * P.hitTargetScaleY * 1.25;
    expect(layout.hitTarget).toEqual({ x: 0, y: 856 - hintH / 2, w: 460, h: hintH });
    expect(layout.stageBottom).toEqual({ x: 230 - 50, y: H - 30, w: 100, h: 30 });
    expect(skinLayout(skin7k(), H, 1).hitTarget).toBeNull();
  });

  it("lays the skin's judgement line one lazer unit thick at the judgement line, or none when disabled", () => {
    expect(skinLayout(skin7k(), H, 1).judgementLine).toEqual({
      x: 0,
      y: 856,
      w: 460,
      h: 1.25,
      fill: { style: "rgb(255, 255, 255)", alpha: P.judgementLineAlpha },
    });
    expect(skinLayout(skin7k({ judgementLine: false }), H, 1).judgementLine).toBeNull();
  });
});

describe("skin colour alpha", () => {
  it("applies column background alpha twice, so alpha 0 hides and 128 reads as (128/255)²", () => {
    expect(doubledAlpha({ r: 0, g: 0, b: 0, a: 0 })).toBe(0);
    expect(doubledAlpha({ r: 0, g: 0, b: 0, a: 128 })).toBeCloseTo((128 / 255) ** 2);
    expect(doubledAlpha({ r: 0, g: 0, b: 0, a: 255 })).toBe(1);
  });

  it("turns alpha 0 into opaque where stable sets the colour after construction", () => {
    expect(opaqueAlpha({ r: 0, g: 0, b: 0, a: 0 })).toBe(1);
    expect(opaqueAlpha({ r: 0, g: 0, b: 0, a: 51 })).toBeCloseTo(0.2);
  });

  it("fills columns with Colour{i+1}, black by default, and lines white by default", () => {
    const layout = skinLayout(
      skin7k({
        colours: {
          column: [{ r: 10, g: 20, b: 30, a: 128 }, null, { r: 1, g: 2, b: 3, a: 0 }],
          columnLine: null,
          judgementLine: { r: 255, g: 0, b: 0, a: 0 },
        },
      }),
      H,
      1,
    );
    expect(layout.columns[0]?.background).toEqual({ style: "rgb(10, 20, 30)", alpha: (128 / 255) ** 2 });
    expect(layout.columns[1]?.background).toEqual({ style: "rgb(0, 0, 0)", alpha: 1 });
    expect(layout.columns[2]?.background.alpha).toBe(0);
    expect(layout.columnLine).toEqual({ style: "rgb(255, 255, 255)", alpha: 1 });
    expect(layout.judgementLine?.fill).toEqual({ style: "rgb(255, 0, 0)", alpha: P.judgementLineAlpha });
  });
});

describe("noteImage", () => {
  it("falls back tail → head → note and head → note", () => {
    const note = image(10, 10);
    const head = image(10, 5);
    const tail = image(10, 2);
    const onlyNote = skin7k({}, [[SKIN_SLOT.note(0), note]]);
    expect(noteImage(onlyNote, 0, "lnHead")).toBe(note);
    expect(noteImage(onlyNote, 0, "lnTail")).toBe(note);
    const withHead = skin7k({}, [
      [SKIN_SLOT.note(0), note],
      [SKIN_SLOT.head(0), head],
    ]);
    expect(noteImage(withHead, 0, "lnTail")).toBe(head);
    const full = skin7k({}, [
      [SKIN_SLOT.note(0), note],
      [SKIN_SLOT.head(0), head],
      [SKIN_SLOT.tail(0), tail],
    ]);
    expect(noteImage(full, 0, "tap")).toBe(note);
    expect(noteImage(full, 0, "lnTail")).toBe(tail);
    expect(noteImage(skin7k(), 0, "tap")).toBeUndefined();
  });
});
