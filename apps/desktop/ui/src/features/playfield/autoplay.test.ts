import { describe, expect, it } from "vitest";
import {
  autoplayFrame,
  autoplayTimeline,
  burstLook,
  comboScaleY,
  DEFAULT_AUTOPLAY_PARAMS as P,
  explosionAlpha,
  lightingFrame,
} from "./autoplay";
import type { ChartNote, ChartWindow } from "./types";

function chart(notes: ChartNote[], fromMs = 1000, toMs = 3000, keymode = 4): ChartWindow {
  return {
    md5: "d41d8cd98f00b204e9800998ecf8427e",
    keymode,
    fromMs,
    toMs,
    notes,
    timing: [],
    layout: { id: "k4", columns: Array.from({ length: keymode }, () => ({ hand: "left" as const, finger: "index" })) },
    chartSpan: { firstMs: 0, endMs: 60_000 },
    audioFilename: null,
  };
}

const at = (window: ChartWindow, nowMs: number) => autoplayFrame(autoplayTimeline(window), nowMs);

describe("autoplay combo", () => {
  const WINDOW = chart([
    { tMs: 1000, col: 0, endMs: null },
    { tMs: 1200, col: 1, endMs: 1600 },
    { tMs: 1400, col: 2, endMs: null },
    { tMs: 900, col: 3, endMs: 1300 },
  ]);

  it("counts every tap, LN head and LN tail once the clock passes it, MAX each", () => {
    expect(at(WINDOW, 1000).combo).toBe(0);
    expect(at(WINDOW, 1000.5).combo).toBe(1);
    expect(at(WINDOW, 1250).combo).toBe(2);
    expect(at(WINDOW, 1350).combo).toBe(3);
    expect(at(WINDOW, 1450).combo).toBe(4);
    expect(at(WINDOW, 1700).combo).toBe(5);
  });

  it("starts from zero at the window start, so the combo resets each time the section loops", () => {
    expect(at(WINDOW, WINDOW.fromMs).combo).toBe(0);
    expect(at(WINDOW, 2999).combo).toBe(5);
    expect(at(WINDOW, WINDOW.fromMs).combo).toBe(0);
  });

  it("reports the time since the latest judgement, or null before the first", () => {
    expect(at(WINDOW, 1000).sinceJudgementMs).toBeNull();
    expect(at(WINDOW, 1030).sinceJudgementMs).toBe(30);
    expect(at(WINDOW, 1310).sinceJudgementMs).toBe(10);
  });
});

describe("autoplay key presses", () => {
  it("holds a tap's key for lazer's release delay and shows the down image 80 ms past the release", () => {
    const window = chart([{ tMs: 1000, col: 0, endMs: null }]);
    expect(at(window, 1000).columns[0]?.keyDown).toBe(false);
    expect(at(window, 1001).columns[0]?.keyDown).toBe(true);
    const release = 1000 + P.releaseDelayMs;
    expect(at(window, release + P.keyUpDelayMs).columns[0]?.keyDown).toBe(true);
    expect(at(window, release + P.keyUpDelayMs + 1).columns[0]?.keyDown).toBe(false);
    expect(at(window, 1001).columns[1]?.keyDown).toBe(false);
  });

  it("holds an LN's key exactly to its tail", () => {
    const window = chart([{ tMs: 1000, col: 2, endMs: 1500 }]);
    expect(at(window, 1499).columns[2]?.keyDown).toBe(true);
    expect(at(window, 1500 + P.keyUpDelayMs + 1).columns[2]?.keyDown).toBe(false);
  });

  it("releases early when the next note in the column is too close, as lazer's autoplay does", () => {
    const tl = autoplayTimeline(
      chart([
        { tMs: 1000, col: 0, endMs: null },
        { tMs: 1010, col: 0, endMs: null },
        { tMs: 1100, col: 1, endMs: 1100 },
      ]),
    );
    expect(tl.presses[0]).toEqual([
      { downMs: 1000, upMs: 1009, holdEndMs: null },
      { downMs: 1010, upMs: 1010 + P.releaseDelayMs, holdEndMs: null },
    ]);
    expect(tl.presses[1]).toEqual([{ downMs: 1100, upMs: 1100 + P.shortHoldReleaseDelayMs, holdEndMs: null }]);
  });

  it("keeps a held LN that began before the window down at the window start", () => {
    const window = chart([{ tMs: 500, col: 1, endMs: 2000 }]);
    expect(at(window, window.fromMs).columns[1]?.keyDown).toBe(true);
    expect(at(window, window.fromMs).combo).toBe(0);
  });
});

describe("autoplay lighting", () => {
  it("lights the stage fully while the key is down, then fades it out over 250 ms", () => {
    const window = chart([{ tMs: 1000, col: 0, endMs: 1500 }]);
    expect(at(window, 1000).columns[0]?.stageLight).toBe(0);
    expect(at(window, 1200).columns[0]?.stageLight).toBe(1);
    expect(at(window, 1500 + P.stageLightFadeMs / 2).columns[0]?.stageLight).toBeCloseTo(0.5);
    expect(at(window, 1500 + P.stageLightFadeMs).columns[0]?.stageLight).toBe(0);
  });

  it("bursts a LightingN explosion at every judgement while it is still visible", () => {
    const window = chart([
      { tMs: 1000, col: 0, endMs: null },
      { tMs: 1100, col: 0, endMs: null },
      { tMs: 1000, col: 1, endMs: 1050 },
    ]);
    const life = P.explosionFadeInMs + P.explosionFadeOutMs;
    expect(at(window, 1150).columns[0]?.explosions).toEqual([150, 50]);
    expect(at(window, 1000 + life + 1).columns[0]?.explosions).toEqual([life + 1 - 100]);
    expect(at(window, 1060).columns[1]?.explosions).toEqual([60, 10]);
  });

  it("shows LightingL while an LN is held, fading in, then out after the tail", () => {
    const window = chart([{ tMs: 1000, col: 0, endMs: 1500 }]);
    expect(at(window, 1000).columns[0]?.hold).toBeNull();
    expect(at(window, 1040).columns[0]?.hold).toEqual({ sinceStartMs: 40, alpha: 40 / P.holdLightFadeInMs });
    expect(at(window, 1400).columns[0]?.hold).toEqual({ sinceStartMs: 400, alpha: 1 });
    expect(at(window, 1500 + P.holdLightFadeOutMs / 2).columns[0]?.hold?.alpha).toBeCloseTo(0.5);
    expect(at(window, 1500 + P.holdLightFadeOutMs).columns[0]?.hold).toBeNull();
    expect(at(window, 1040).columns[1]?.hold).toBeNull();
  });
});

describe("effect curves", () => {
  it("fades a burst in over 20 ms, holds it 160 ms and fades it out over 40 ms, scaling like stable", () => {
    expect(burstLook(0)).toBeNull();
    expect(burstLook(10)?.alpha).toBeCloseTo(1 - 0.5 ** 2);
    expect(burstLook(20)).toEqual({ alpha: 1, scale: expect.closeTo(0.9) as number });
    expect(burstLook(40)?.scale).toBeCloseTo(0.85);
    expect(burstLook(100)).toEqual({ alpha: 1, scale: 0.7 });
    expect(burstLook(200)?.alpha).toBeCloseTo(1 - 0.5 ** 2);
    expect(burstLook(200)?.scale).toBeCloseTo(0.7 - 0.3 * 0.5 ** 2);
    expect(burstLook(220)).toBeNull();
  });

  it("takes a burst's scale keyframes from the params", () => {
    const p = {
      ...P,
      burstScaleStepMs: 10,
      burstPopScale: [0.5, 2] as const,
      burstSettleScale: [1.5, 1] as const,
      burstEndScale: 0.25,
    };
    expect(burstLook(5, p)?.scale).toBeCloseTo(1.25);
    expect(burstLook(15, p)?.scale).toBeCloseTo(1.25);
    expect(burstLook(100, p)?.scale).toBe(1);
    expect(burstLook(200, p)?.scale).toBeCloseTo(1 - 0.75 * 0.5 ** 2);
  });

  it("fades an explosion in over 80 ms and out over 120 ms", () => {
    expect(explosionAlpha(40)).toBeCloseTo(0.5);
    expect(explosionAlpha(80)).toBe(1);
    expect(explosionAlpha(140)).toBeCloseTo(0.5);
    expect(explosionAlpha(200)).toBe(0);
  });

  it("pops the combo 1.4× tall and eases it back over 300 ms", () => {
    expect(comboScaleY(0)).toBeCloseTo(1.4);
    expect(comboScaleY(150)).toBeCloseTo(1 + 0.4 * 0.25);
    expect(comboScaleY(300)).toBe(1);
    expect(comboScaleY(5000)).toBe(1);
  });

  it("steps lighting frames at max(1000/60, 170/count) ms, once for taps and looping for holds", () => {
    expect(lightingFrame(0, 10, false)).toBe(0);
    expect(lightingFrame(17, 10, false)).toBe(1);
    expect(lightingFrame(1000, 10, false)).toBe(9);
    expect(lightingFrame(3 * 42.5 + 1, 4, true)).toBe(3);
    expect(lightingFrame(4 * 42.5 + 1, 4, true)).toBe(0);
    expect(lightingFrame(5, 0, true)).toBe(0);
  });
});
