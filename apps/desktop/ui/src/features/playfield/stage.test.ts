import { describe, expect, it } from "vitest";
import {
  clampOsuSpeed,
  clampZoom,
  DEFAULT_STAGE_PARAMS as P,
  judgeYFromHitPosition,
  osuPxPerMs,
  scrollPxPerMs,
  stageWidthPx,
  uniformColumnWidths,
  visibleMs,
} from "./stage";

const HEIGHTS = [480, 600, 777, 1080];

describe("osu! stable scroll speed", () => {
  it("moves 0.035·n px of the 480-px space per ms, scaled to the canvas height", () => {
    expect(osuPxPerMs(30, 480)).toBeCloseTo(1.05);
    expect(osuPxPerMs(30, 960)).toBeCloseTo(2.1);
    expect(osuPxPerMs(1, 480)).toBeCloseTo(P.osuPxPerMsPer480);
  });

  it.each(HEIGHTS)("shows 407.6 ms above the judgement line at speed 30 with HitPosition 428 (height %i)", (height) => {
    const ms = visibleMs(osuPxPerMs(30, height), judgeYFromHitPosition(428, height));
    expect(Math.abs(ms - 407.6)).toBeLessThanOrEqual(0.1);
  });

  it.each([
    [1, 12228.6],
    [10, 1222.9],
    [20, 611.4],
    [40, 305.7],
  ])("matches 428/(0.035·n) at speed %i", (speed, expected) => {
    for (const height of HEIGHTS) {
      const ms = visibleMs(osuPxPerMs(speed, height), judgeYFromHitPosition(428, height));
      expect(Math.abs(ms - expected)).toBeLessThanOrEqual(0.1);
    }
  });

  // Research 06, "Scroll speed": v = 0.035·n is fixed, so T = HitPosition / (0.035·n) = HitPosition·200/(7n).
  it.each([240, 402, 428, 465, 480])(
    "takes HitPosition·200/(7n) ms from the top edge to HitPosition %i, the velocity being the same for every HitPosition",
    (hitPosition) => {
      for (const speed of [1, 10, 30, 40]) {
        for (const height of HEIGHTS) {
          const ms = visibleMs(osuPxPerMs(speed, height), judgeYFromHitPosition(hitPosition, height));
          expect(ms).toBeCloseTo((hitPosition * 200) / (7 * speed), 6);
        }
      }
    },
  );

  it("matches stable's SpeedMania.TimeAt at the default HitPosition (ppy/osu PR #13901)", () => {
    expect(visibleMs(osuPxPerMs(40, 480), judgeYFromHitPosition(402, 480))).toBeCloseTo(287.14, 2);
    expect(visibleMs(osuPxPerMs(1, 480), judgeYFromHitPosition(402, 480))).toBeCloseTo(11485.71, 2);
  });

  it("clamps the speed to 1..40 in whole steps", () => {
    expect(clampOsuSpeed(0)).toBe(1);
    expect(clampOsuSpeed(-5)).toBe(1);
    expect(clampOsuSpeed(41)).toBe(40);
    expect(clampOsuSpeed(29.6)).toBe(30);
    expect(clampOsuSpeed(Number.NaN)).toBe(P.minOsuSpeed);
  });
});

describe("scrollPxPerMs", () => {
  it("converts either mode to map-time px/ms, dividing the real-time velocity by the rate", () => {
    expect(scrollPxPerMs({ kind: "osu", speed: 30 }, 480)).toBeCloseTo(1.05);
    expect(scrollPxPerMs({ kind: "pxPerMs", value: 0.8 }, 480)).toBeCloseTo(0.8);
    expect(scrollPxPerMs({ kind: "osu", speed: 30 }, 480, 1.5)).toBeCloseTo(0.7);
    expect(scrollPxPerMs({ kind: "pxPerMs", value: 0.8 }, 480, 0.5)).toBeCloseTo(1.6);
  });

  it("clamps an out-of-range osu! speed", () => {
    expect(scrollPxPerMs({ kind: "osu", speed: 99 }, 480)).toBeCloseTo(osuPxPerMs(40, 480));
  });
});

describe("judgeYFromHitPosition", () => {
  it("places the judgement line at HitPosition of the 480-px space, clamped to 240..480", () => {
    expect(judgeYFromHitPosition(402, 480)).toBe(402);
    expect(judgeYFromHitPosition(428, 960)).toBe(856);
    expect(judgeYFromHitPosition(100, 480)).toBe(240);
    expect(judgeYFromHitPosition(600, 480)).toBe(480);
    expect(P.defaultHitPosition).toBe(402);
  });
});

describe("stage width", () => {
  it("sums the 480-space column widths scaled by the height and the zoom", () => {
    const widths = uniformColumnWidths(7);
    expect(widths).toEqual(Array.from({ length: 7 }, () => P.defaultColumnWidth));
    expect(stageWidthPx(widths, 480, 1)).toBe(210);
    expect(stageWidthPx(widths, 960, 1)).toBe(420);
    expect(stageWidthPx(widths, 960, 1.5)).toBe(630);
    expect(stageWidthPx([42, 42, 42, 42, 42, 42, 42], 480, 1)).toBe(294);
  });

  it("clamps the zoom to 0.75..2", () => {
    expect(clampZoom(0.1)).toBe(0.75);
    expect(clampZoom(5)).toBe(2);
    expect(clampZoom(1.25)).toBe(1.25);
    expect(clampZoom(Number.NaN)).toBe(1);
  });
});
