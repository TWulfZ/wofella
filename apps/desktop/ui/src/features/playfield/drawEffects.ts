// The preview's playback effects drawn over the playfield, after lazer's legacy mania pieces (ppy/osu @6359741b, MIT;
// research 06): judgement bursts, the combo counter, key presses and lighting, for the autoplay frame of the moment.

import {
  type AutoplayFrame,
  burstLook,
  comboScaleY,
  DEFAULT_AUTOPLAY_PARAMS,
  explosionAlpha,
  lightingFrame,
  type AutoplayParams,
} from "./autoplay";
import type { Draw2D, PlayfieldTheme } from "./draw";
import type { PlayfieldEffects } from "./effects";
import { displayHeight, displayWidth, type SkinLayout, type SkinLayoutParams, type SkinRect } from "./skinLayout";
import { type HitResultName, type LoadedSkin, SKIN_SLOT, type SkinImage, type SkinSlot } from "./skinModel";
import { DEFAULT_STAGE_PARAMS } from "./stage";

export interface PlayfieldFx {
  flags: PlayfieldEffects;
  /** Null when no autoplay effect is on; the percy switch needs no frame. */
  frame: AutoplayFrame | null;
}

export interface EffectDrawParams {
  /** `LegacyManiaSkinConfiguration.DEFAULT_COLUMN_SIZE` in stable units: lighting scales by column width over it. */
  lightingBaseColumnWidth: number;
  /** `LegacySkin.cs` L293–294, L304–305: lighting is not scaled before this Version. */
  lightingScaleFromVersion: number;
  /** A MAX uses the first burst the skin has: the autoplay judges nothing below MAX, so any other art stands in. */
  maxBurstChain: readonly HitResultName[];
  /** Procedural stand-in for a MAX burst; a game term, the same in every locale. */
  maxText: string;
  /** Text sizes in stable's 480-high units. */
  judgementTextUnits: number;
  comboTextUnits: number;
  textFamily: string;
  /** Where the procedural stage puts the text: lazer's defaults (`LegacyManiaSkinConfiguration.cs` L37–38). */
  defaultScorePosition: number;
  defaultComboPosition: number;
  virtualHeightPx: number;
  autoplay: AutoplayParams;
}

export const DEFAULT_EFFECT_DRAW_PARAMS: EffectDrawParams = {
  lightingBaseColumnWidth: 30,
  lightingScaleFromVersion: 2.5,
  maxBurstChain: ["300g", "300", "200", "100", "50"],
  maxText: "MAX",
  judgementTextUnits: 24,
  comboTextUnits: 32,
  textFamily: "system-ui, sans-serif",
  defaultScorePosition: 300,
  defaultComboPosition: 111,
  virtualHeightPx: DEFAULT_STAGE_PARAMS.virtualHeightPx,
  autoplay: DEFAULT_AUTOPLAY_PARAMS,
};

function blit(ctx: Draw2D, img: SkinImage, r: SkinRect, alpha: number, composite: GlobalCompositeOperation): void {
  ctx.globalAlpha = alpha;
  ctx.globalCompositeOperation = composite;
  ctx.drawImage(img.bitmap, 0, 0, img.width, img.height, r.x, r.y, r.w, r.h);
  ctx.globalCompositeOperation = "source-over";
  ctx.globalAlpha = 1;
}

/** `LegacyKeyArea.cs` L92–110: the down image replaces the up one while the column is held; same width, own height. */
export function keySprite(
  skin: LoadedSkin,
  layout: SkinLayout,
  col: number,
  fx: PlayfieldFx | undefined,
  params: SkinLayoutParams,
): { img: SkinImage; rect: SkinRect } | null {
  const column = layout.columns[col];
  const up = skin.images.get(SKIN_SLOT.key(col));
  const down = skin.images.get(SKIN_SLOT.keyDown(col));
  if (column === undefined) {
    return null;
  }
  if (fx?.flags.keyPress === true && fx.frame?.columns[col]?.keyDown === true && down !== undefined) {
    const h = displayHeight(down) * layout.k * params.keyHeightPerK;
    return { img: down, rect: { x: column.x, y: layout.height - h, w: column.w, h } };
  }
  return up === undefined || column.key === null ? null : { img: up, rect: column.key };
}

/**
 * `LegacyColumnBackground.cs` L43–60, L80–100: the column's light stands on LightPosition at full alpha while the key
 * is down, then fades and shrinks toward its bottom. Drawn above the hit target, under the notes.
 */
export function drawStageLights(
  ctx: Draw2D,
  layout: SkinLayout,
  skin: LoadedSkin,
  fx: PlayfieldFx | undefined,
  params: EffectDrawParams = DEFAULT_EFFECT_DRAW_PARAMS,
): void {
  const frame = fx?.frame;
  if (fx?.flags.lighting !== true || frame === null || frame === undefined) {
    return;
  }
  const bottom = skin.lightPosition * unzoomedK(layout, params);
  for (const [i, column] of layout.columns.entries()) {
    const strength = frame.columns[i]?.stageLight ?? 0;
    const img = skin.stageLights.get(i) ?? skin.images.get(SKIN_SLOT.stageLight);
    if (strength <= 0 || img === undefined) {
      continue;
    }
    const h = displayHeight(img) * layout.unit * strength;
    blit(ctx, img, { x: column.x, y: bottom - h, w: column.w, h }, strength, "source-over");
  }
}

/** Zoom only widens the stage (`skinLayout`), so heights measured from the top must ignore it, as judgeY does. */
function unzoomedK(layout: SkinLayout, params: EffectDrawParams): number {
  return layout.height / params.virtualHeightPx;
}

function frames(skin: LoadedSkin, slot: (frame: number) => SkinSlot): SkinImage[] {
  const out: SkinImage[] = [];
  for (let f = 0; ; f++) {
    const img = skin.images.get(slot(f));
    if (img === undefined) {
      return out;
    }
    out.push(img);
  }
}

/** Skinned effects drawn last, over notes, keys and the shade: lighting, then the burst, then the combo. */
export function drawSkinEffects(
  ctx: Draw2D,
  layout: SkinLayout,
  skin: LoadedSkin,
  theme: PlayfieldTheme,
  fx: PlayfieldFx | undefined,
  params: EffectDrawParams = DEFAULT_EFFECT_DRAW_PARAMS,
): void {
  const frame = fx?.frame;
  if (fx === undefined || frame === null || frame === undefined) {
    return;
  }
  const centreX = layout.stageX + layout.stageWidth / 2;
  if (fx.flags.lighting) {
    drawLighting(ctx, layout, skin, frame, params);
  }
  if (fx.flags.judgements) {
    const look = frame.sinceJudgementMs === null ? null : burstLook(frame.sinceJudgementMs, params.autoplay);
    const cy = skin.effects.scorePosition * unzoomedK(layout, params);
    const burst = params.maxBurstChain.map((r) => skin.images.get(SKIN_SLOT.hit(r))).find((img) => img !== undefined);
    if (look !== null && burst !== undefined) {
      const w = displayWidth(burst) * layout.unit * look.scale;
      const h = displayHeight(burst) * layout.unit * look.scale;
      blit(ctx, burst, { x: centreX - w / 2, y: cy - h / 2, w, h }, look.alpha, "source-over");
    } else if (look !== null) {
      burstText(ctx, centreX, cy, layout.k, look, theme, params);
    }
  }
  if (fx.flags.combo && frame.combo > 0) {
    drawCombo(ctx, layout, skin, theme, frame, centreX, params);
  }
}

function drawLighting(ctx: Draw2D, layout: SkinLayout, skin: LoadedSkin, frame: AutoplayFrame, params: EffectDrawParams): void {
  const lightingN = frames(skin, SKIN_SLOT.lightingN);
  const lightingL = frames(skin, SKIN_SLOT.lightingL);
  const scaled = skin.version >= params.lightingScaleFromVersion;
  const scaleOf = (widths: readonly number[], col: number): number => {
    const w = widths[col] ?? 0;
    return scaled ? (w !== 0 ? w : (skin.columnWidth[col] ?? 0)) / params.lightingBaseColumnWidth : 1;
  };
  // `LegacyHitExplosion.cs` L46–51, `LegacyBodyPiece.cs` L61–66: centred on the judgement line, additive.
  const centred = (img: SkinImage, col: number, scale: number): SkinRect => {
    const column = layout.columns[col];
    const w = displayWidth(img) * layout.unit * scale;
    const h = displayHeight(img) * layout.unit * scale;
    const cx = column === undefined ? 0 : column.x + column.w / 2;
    return { x: cx - w / 2, y: layout.judgeY - h / 2, w, h };
  };
  for (const [col, state] of frame.columns.entries()) {
    if (state.hold !== null && lightingL.length > 0) {
      const img = lightingL[lightingFrame(state.hold.sinceStartMs, lightingL.length, true, params.autoplay)];
      if (img !== undefined) {
        blit(ctx, img, centred(img, col, scaleOf(skin.effects.lightingLWidth, col)), state.hold.alpha, "lighter");
      }
    }
    for (const age of state.explosions) {
      const img = lightingN[lightingFrame(age, lightingN.length, false, params.autoplay)];
      const alpha = explosionAlpha(age, params.autoplay);
      if (img !== undefined && alpha > 0) {
        blit(ctx, img, centred(img, col, scaleOf(skin.effects.lightingNWidth, col)), alpha, "lighter");
      }
    }
  }
}

/** `LegacyManiaComboCounter.cs`, `LegacySpriteText.cs` L53–55: digits side by side, `ComboOverlap` px closer. */
function drawCombo(
  ctx: Draw2D,
  layout: SkinLayout,
  skin: LoadedSkin,
  theme: PlayfieldTheme,
  frame: AutoplayFrame,
  centreX: number,
  params: EffectDrawParams,
): void {
  const text = String(frame.combo);
  const cy = skin.effects.comboPosition * unzoomedK(layout, params);
  const scaleY = comboScaleY(frame.sinceJudgementMs ?? Infinity, params.autoplay);
  const glyphs = Array.from(text, (d) => skin.images.get(SKIN_SLOT.comboDigit(Number(d))));
  if (glyphs.some((g) => g === undefined)) {
    comboText(ctx, text, centreX, cy, layout.k, scaleY, theme, params);
    return;
  }
  const images = glyphs as SkinImage[];
  const overlap = skin.effects.comboOverlap * layout.unit;
  const widths = images.map((img) => displayWidth(img) * layout.unit);
  const total = widths.reduce((sum, w) => sum + w, 0) - overlap * (images.length - 1);
  let x = centreX - total / 2;
  for (const [i, img] of images.entries()) {
    const w = widths[i] ?? 0;
    const h = displayHeight(img) * layout.unit * scaleY;
    blit(ctx, img, { x, y: cy - h / 2, w, h }, 1, "source-over");
    x += w - overlap;
  }
}

function text(
  ctx: Draw2D,
  value: string,
  at: { x: number; y: number; sx: number; sy: number },
  px: number,
  fill: string,
  alpha: number,
  params: EffectDrawParams,
): void {
  ctx.save();
  ctx.globalAlpha = alpha;
  ctx.fillStyle = fill;
  ctx.font = `bold ${px}px ${params.textFamily}`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.translate(at.x, at.y);
  ctx.scale(at.sx, at.sy);
  ctx.fillText(value, 0, 0);
  ctx.restore();
}

function burstText(
  ctx: Draw2D,
  x: number,
  y: number,
  k: number,
  look: { alpha: number; scale: number },
  theme: PlayfieldTheme,
  params: EffectDrawParams,
): void {
  const at = { x, y, sx: look.scale, sy: look.scale };
  text(ctx, params.maxText, at, params.judgementTextUnits * k, theme.maxText, look.alpha, params);
}

function comboText(
  ctx: Draw2D,
  value: string,
  x: number,
  y: number,
  k: number,
  scaleY: number,
  theme: PlayfieldTheme,
  params: EffectDrawParams,
): void {
  text(ctx, value, { x, y, sx: 1, sy: scaleY }, params.comboTextUnits * k, theme.comboText, 1, params);
}

/** The procedural stage has no skin art: judgements and combo as text at lazer's default positions. */
export function drawTextEffects(
  ctx: Draw2D,
  view: { width: number; height: number },
  theme: PlayfieldTheme,
  fx: PlayfieldFx | undefined,
  params: EffectDrawParams = DEFAULT_EFFECT_DRAW_PARAMS,
): void {
  const frame = fx?.frame;
  if (fx === undefined || frame === null || frame === undefined) {
    return;
  }
  const k = view.height / params.virtualHeightPx;
  const x = view.width / 2;
  if (fx.flags.judgements && frame.sinceJudgementMs !== null) {
    const look = burstLook(frame.sinceJudgementMs, params.autoplay);
    if (look !== null) {
      burstText(ctx, x, params.defaultScorePosition * k, k, look, theme, params);
    }
  }
  if (fx.flags.combo && frame.combo > 0) {
    const scaleY = comboScaleY(frame.sinceJudgementMs ?? Infinity, params.autoplay);
    comboText(ctx, String(frame.combo), x, params.defaultComboPosition * k, k, scaleY, theme, params);
  }
}
