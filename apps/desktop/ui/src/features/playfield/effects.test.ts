import { describe, expect, it } from "vitest";
import { DEFAULT_PLAYFIELD_EFFECTS, skinEffectSupport } from "./effects";
import { DEFAULT_SKIN_LAYOUT_PARAMS } from "./skinLayout";
import { SKIN_SLOT, type SkinSlot } from "./skinModel";
import { image, skin7k } from "./testSkin";

describe("playfield effects contract", () => {
  it("draws percy as before and leaves the autoplay effects off by default", () => {
    expect(DEFAULT_PLAYFIELD_EFFECTS).toEqual({
      percy: true,
      judgements: false,
      combo: false,
      keyPress: false,
      lighting: false,
    });
  });

  it("offers text judgements and combo on the procedural stage, and nothing a skin would have to provide", () => {
    expect(skinEffectSupport(null)).toEqual({
      percy: false,
      judgements: true,
      combo: true,
      keyPress: false,
      lighting: false,
    });
  });

  it("supports nothing for a skin that has none of the images", () => {
    expect(skinEffectSupport(skin7k())).toEqual({
      percy: false,
      judgements: false,
      combo: false,
      keyPress: false,
      lighting: false,
    });
  });

  it.each<[keyof typeof DEFAULT_PLAYFIELD_EFFECTS, SkinSlot]>([
    ["judgements", SKIN_SLOT.hit("300g")],
    ["judgements", SKIN_SLOT.hit("50")],
    ["combo", SKIN_SLOT.comboDigit(0)],
    ["keyPress", SKIN_SLOT.keyDown(4)],
    ["lighting", SKIN_SLOT.lightingN(0)],
    ["lighting", SKIN_SLOT.lightingL(0)],
    ["lighting", SKIN_SLOT.stageLight],
  ])("supports %s once the skin has %s", (effect, slot) => {
    const support = skinEffectSupport(skin7k({}, [[slot, image(10, 10)]]));
    expect(support[effect]).toBe(true);
    expect(Object.values(support).filter(Boolean)).toHaveLength(1);
  });

  it("finds no judgement art in a skin whose only hit image is the miss, which a MAX never shows", () => {
    expect(skinEffectSupport(skin7k({}, [[SKIN_SLOT.hit("0"), image(10, 10)]])).judgements).toBe(false);
  });

  it("treats a combo font as present only when its 0 digit is, as lazer's HasFont does", () => {
    expect(skinEffectSupport(skin7k({}, [[SKIN_SLOT.comboDigit(7), image(8, 12)]])).combo).toBe(false);
  });

  it("finds percy in a strip-shaped or cropped LN body, not in a tiled one", () => {
    const aspect = DEFAULT_SKIN_LAYOUT_PARAMS.stripBodyAspect;
    const percy = (body: ReturnType<typeof image>) => skinEffectSupport(skin7k({}, [[SKIN_SLOT.body(2), body]])).percy;
    expect(percy(image(138, 138 * aspect))).toBe(true);
    expect(percy(image(138, 4096, 1, 40_000))).toBe(true);
    expect(percy(image(30, 30))).toBe(false);
  });
});
