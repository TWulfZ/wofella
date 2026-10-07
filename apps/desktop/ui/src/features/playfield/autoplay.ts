// A perfect autoplay of the chart window for the preview's skin effects, after lazer's mania autoplay and legacy pieces
// (ppy/osu @6359741b, MIT). Every state is a pure function of (window, nowMs): the drawing never reads a wall clock,
// so the preview stays deterministic and a looped section replays identically.

import type { ChartWindow } from "./types";

export interface AutoplayParams {
  /** `ManiaAutoGenerator.cs` L16, L71–86: a tap's key is released this long after the note. */
  releaseDelayMs: number;
  /** `ManiaAutoGenerator.cs` L79–80: zero-length hold notes. */
  shortHoldReleaseDelayMs: number;
  /** `ManiaAutoGenerator.cs` L86: release this share of the way to a next note that is too close. */
  nextNoteReleaseFraction: number;
  /** `LegacyKeyArea.cs` L107–108: the down image stays `LegacyHitExplosion.FADE_IN_DURATION` past the release. */
  keyUpDelayMs: number;
  /** `LegacyColumnBackground.cs` L94–99: the stage light fades and shrinks after the release. */
  stageLightFadeMs: number;
  /** `LegacyHitExplosion.cs` L19, L69–70. */
  explosionFadeInMs: number;
  explosionFadeOutMs: number;
  /** `LegacyBodyPiece.cs` L132, L136. */
  holdLightFadeInMs: number;
  holdLightFadeOutMs: number;
  /** `LegacyHitExplosion.cs` L44, `LegacyBodyPiece.cs` L59: frame length is max(1000/60, 170/frames) ms. */
  lightingMinFrameMs: number;
  lightingCycleMs: number;
  /** `LegacyManiaJudgementPiece.cs` L73–75. */
  burstFadeInMs: number;
  burstHoldMs: number;
  burstFadeOutMs: number;
  /**
   * `LegacyManiaJudgementPiece.cs` L89–96: the scale pops from→to over one step, jumps and settles from→to over the
   * next, holds, then eases to the end scale during the fade-out. lazer keeps stable's overlapping transforms.
   */
  burstScaleStepMs: number;
  burstPopScale: readonly [number, number];
  burstSettleScale: readonly [number, number];
  burstEndScale: number;
  /** `LegacyManiaComboCounter.cs` L153–154. */
  comboPopScaleY: number;
  comboPopMs: number;
}

export const DEFAULT_AUTOPLAY_PARAMS: AutoplayParams = {
  releaseDelayMs: 20,
  shortHoldReleaseDelayMs: 1,
  nextNoteReleaseFraction: 0.9,
  keyUpDelayMs: 80,
  stageLightFadeMs: 250,
  explosionFadeInMs: 80,
  explosionFadeOutMs: 120,
  holdLightFadeInMs: 80,
  holdLightFadeOutMs: 120,
  lightingMinFrameMs: 1000 / 60,
  lightingCycleMs: 170,
  burstFadeInMs: 20,
  burstHoldMs: 160,
  burstFadeOutMs: 40,
  burstScaleStepMs: 40,
  burstPopScale: [0.8, 1],
  burstSettleScale: [0.85, 0.7],
  burstEndScale: 0.4,
  comboPopScaleY: 1.4,
  comboPopMs: 300,
};

export interface AutoplayPress {
  downMs: number;
  upMs: number;
  /** The LN's tail; null for a tap. */
  holdEndMs: number | null;
}

export interface AutoplayTimeline {
  fromMs: number;
  /** Ascending. Taps, LN heads and LN tails are each judged MAX (lazer judges heads and tails apart). */
  judgements: { tMs: number; col: number }[];
  /** Per column, ascending. */
  presses: AutoplayPress[][];
  params: AutoplayParams;
}

export interface AutoplayColumn {
  keyDown: boolean;
  /** 1 while the key is down, falling to 0 after the release. */
  stageLight: number;
  /** Ages of the LightingN explosions still visible, oldest first. */
  explosions: number[];
  /** LightingL while an LN is held and fading out after it; null when not visible. */
  hold: { sinceStartMs: number; alpha: number } | null;
}

export interface AutoplayFrame {
  /** Judgements since the window start: the section's loop restarts it from 0. */
  combo: number;
  /** ms since the latest judgement; null before the first. */
  sinceJudgementMs: number | null;
  columns: AutoplayColumn[];
}

export function autoplayTimeline(window: ChartWindow, params: AutoplayParams = DEFAULT_AUTOPLAY_PARAMS): AutoplayTimeline {
  const byColumn: ChartWindow["notes"][] = Array.from({ length: window.keymode }, () => []);
  const judgements: AutoplayTimeline["judgements"] = [];
  for (const note of window.notes) {
    byColumn[note.col]?.push(note);
    judgements.push({ tMs: note.tMs, col: note.col });
    if (note.endMs !== null) {
      judgements.push({ tMs: note.endMs, col: note.col });
    }
  }
  judgements.sort((a, b) => a.tMs - b.tMs);

  const presses = byColumn.map((notes) => {
    const sorted = [...notes].sort((a, b) => a.tMs - b.tMs);
    return sorted.map((note, i): AutoplayPress => {
      const endMs = note.endMs ?? note.tMs;
      const isHold = note.endMs !== null;
      if (isHold && endMs > note.tMs) {
        return { downMs: note.tMs, upMs: endMs, holdEndMs: endMs };
      }
      const delay = isHold ? params.shortHoldReleaseDelayMs : params.releaseDelayMs;
      const next = sorted[i + 1];
      const upMs =
        next === undefined || next.tMs > endMs + delay
          ? endMs + delay
          : endMs + (next.tMs - endMs) * params.nextNoteReleaseFraction;
      return { downMs: note.tMs, upMs, holdEndMs: null };
    });
  });
  return { fromMs: window.fromMs, judgements, presses, params };
}

/** Anything at time t has happened once the clock is past t, so a paused frame at the window start shows none of it. */
export function autoplayFrame(timeline: AutoplayTimeline, nowMs: number): AutoplayFrame {
  const p = timeline.params;
  const explosionLife = p.explosionFadeInMs + p.explosionFadeOutMs;
  let combo = 0;
  let latest: number | null = null;
  const columns: AutoplayColumn[] = timeline.presses.map(() => ({
    keyDown: false,
    stageLight: 0,
    explosions: [],
    hold: null,
  }));

  for (const j of timeline.judgements) {
    if (j.tMs >= nowMs) {
      break;
    }
    if (j.tMs >= timeline.fromMs) {
      combo++;
      latest = j.tMs;
    }
    const age = nowMs - j.tMs;
    if (age <= explosionLife) {
      columns[j.col]?.explosions.push(age);
    }
  }

  for (const [col, presses] of timeline.presses.entries()) {
    const column = columns[col];
    if (column === undefined) {
      continue;
    }
    let last: AutoplayPress | undefined;
    for (const press of presses) {
      if (press.downMs >= nowMs) {
        break;
      }
      last = press;
      if (nowMs <= press.upMs + p.keyUpDelayMs) {
        column.keyDown = true;
      }
      if (press.holdEndMs !== null) {
        column.hold = holdLight(press, press.holdEndMs, nowMs, p) ?? column.hold;
      }
    }
    if (last !== undefined) {
      column.stageLight = nowMs <= last.upMs ? 1 : Math.max(0, 1 - (nowMs - last.upMs) / p.stageLightFadeMs);
    }
  }

  return { combo, sinceJudgementMs: latest === null ? null : nowMs - latest, columns };
}

function holdLight(press: AutoplayPress, endMs: number, nowMs: number, p: AutoplayParams): AutoplayColumn["hold"] {
  const fadeIn = (t: number): number => Math.min(1, (t - press.downMs) / p.holdLightFadeInMs);
  const sinceStartMs = nowMs - press.downMs;
  if (nowMs <= endMs) {
    return { sinceStartMs, alpha: fadeIn(nowMs) };
  }
  const out = (nowMs - endMs) / p.holdLightFadeOutMs;
  return out < 1 ? { sinceStartMs, alpha: fadeIn(endMs) * (1 - out) } : null;
}

// osu-framework's Easing.In and Easing.Out are InQuad and OutQuad (`DefaultEasingFunction.cs` L46–51).
const inQuad = (t: number): number => t * t;
const outQuad = (t: number): number => 1 - (1 - t) * (1 - t);
const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;

/** A judgement burst's alpha and scale `ageMs` after it, null once gone. */
export function burstLook(ageMs: number, p: AutoplayParams = DEFAULT_AUTOPLAY_PARAMS): { alpha: number; scale: number } | null {
  const outStart = p.burstFadeInMs + p.burstHoldMs;
  const end = outStart + p.burstFadeOutMs;
  if (ageMs <= 0 || ageMs >= end) {
    return null;
  }
  const alpha =
    ageMs < p.burstFadeInMs
      ? outQuad(ageMs / p.burstFadeInMs)
      : ageMs < outStart
        ? 1
        : 1 - inQuad((ageMs - outStart) / p.burstFadeOutMs);
  const step = p.burstScaleStepMs;
  const [popFrom, popTo] = p.burstPopScale;
  const [settleFrom, settled] = p.burstSettleScale;
  let scale: number;
  if (ageMs < step) {
    scale = lerp(popFrom, popTo, ageMs / step);
  } else if (ageMs < 2 * step) {
    scale = lerp(settleFrom, settled, (ageMs - step) / step);
  } else if (ageMs < outStart) {
    scale = settled;
  } else {
    scale = lerp(settled, p.burstEndScale, inQuad((ageMs - outStart) / p.burstFadeOutMs));
  }
  return { alpha, scale };
}

export function explosionAlpha(ageMs: number, p: AutoplayParams = DEFAULT_AUTOPLAY_PARAMS): number {
  if (ageMs <= 0) {
    return 0;
  }
  if (ageMs < p.explosionFadeInMs) {
    return ageMs / p.explosionFadeInMs;
  }
  return Math.max(0, 1 - (ageMs - p.explosionFadeInMs) / p.explosionFadeOutMs);
}

export function comboScaleY(sinceMs: number, p: AutoplayParams = DEFAULT_AUTOPLAY_PARAMS): number {
  if (sinceMs >= p.comboPopMs) {
    return 1;
  }
  return lerp(p.comboPopScaleY, 1, outQuad(Math.max(0, sinceMs) / p.comboPopMs));
}

/** LightingN plays once and holds its last frame; LightingL loops while held (`GetAnimation` looping flags). */
export function lightingFrame(ageMs: number, count: number, loop: boolean, p: AutoplayParams = DEFAULT_AUTOPLAY_PARAMS): number {
  if (count <= 0) {
    return 0;
  }
  const frameMs = Math.max(p.lightingMinFrameMs, p.lightingCycleMs / count);
  const n = Math.floor(Math.max(0, ageMs) / frameMs);
  return loop ? n % count : Math.min(n, count - 1);
}
