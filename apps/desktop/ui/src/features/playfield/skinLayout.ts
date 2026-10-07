// Geometry of a legacy mania skin on the canvas, after lazer's legacy pieces (ppy/osu @6359741b,
// osu.Game.Rulesets.Mania/Skinning/Legacy/*); the rules and their unverified parts are in research 06.

import { DEFAULT_PLAYFIELD_PARAMS } from "./project";
import { type LoadedSkin, SKIN_SLOT, type SkinImage, type SkinRgba } from "./skinModel";
import { clampZoom, DEFAULT_STAGE_PARAMS, judgeYFromHitPosition, type StageParams } from "./stage";

export interface SkinLayoutParams {
  /** lazer's 768-high space over stable's 480 (`LegacySkin.STABLE_MAGIC_SCALE_FACTOR`). */
  stableMagicScale: number;
  /** `LegacyStageBackground.cs` L121, L134. */
  columnLineScaleX: number;
  /** `LegacyStageBackground.cs`: the last column's right line is pulled in by this many lazer units. */
  lastColumnLineOffsetUnits: number;
  /** `LegacyStageBackground.cs` L101: right column lines exist from this Version on (always on the last column). */
  rightLinesFromVersion: number;
  /** `LegacyHitTarget.cs` L45: 0.9 × 1.6025. */
  hitTargetScaleY: number;
  /** `LegacyHitTarget.cs` L53–55. */
  judgementLineAlpha: number;
  judgementLineUnits: number;
  /** `LegacyStageForeground.cs`: stage-bottom is drawn at 1.6× its display size, in lazer units. */
  stageBottomScale: number;
  /**
   * Unverified (research 06): lazer draws keys at their display height in its 768-high space, i.e. × k/1.6;
   * wolluf uses × k until the pilot's side-by-side check settles it.
   */
  keyHeightPerK: number;
  /** A repeated body thinner than this is stretched: it reads as a solid fill, and tiling costs a draw per px. */
  minBodyTilePx: number;
  /**
   * A body at least this many times taller than wide is a strip (percy): drawn once from the tail, never re-tiled,
   * or its lead-in and cap would reappear mid-hold (research 06).
   */
  stripBodyAspect: number;
  /** A strip's last source rows, stretched over the part of a hold longer than the strip. */
  stripTailRows: number;
  /** Images shorter than this are skipped: invisible, yet each still costs a draw (percy's 1-row tail). */
  minImageDrawPx: number;
  /** Procedural sizes for slots the skin does not resolve, the same as the procedural playfield. */
  fallbackNoteHeightPx: number;
  fallbackTailHeightPx: number;
  /** Defaults when skin.ini sets no colour (`LegacyStageBackground.cs`, `LegacyHitTarget.cs`). */
  columnColour: SkinRgba;
  columnLineColour: SkinRgba;
  judgementLineColour: SkinRgba;
}

export const DEFAULT_SKIN_LAYOUT_PARAMS: SkinLayoutParams = {
  stableMagicScale: 1.6,
  columnLineScaleX: 0.74,
  lastColumnLineOffsetUnits: -0.16,
  rightLinesFromVersion: 2.4,
  hitTargetScaleY: 0.9 * 1.6025,
  judgementLineAlpha: 0.9,
  judgementLineUnits: 1,
  stageBottomScale: 1.6,
  keyHeightPerK: 1,
  minBodyTilePx: 2,
  stripBodyAspect: 8,
  stripTailRows: 8,
  minImageDrawPx: 0.5,
  fallbackNoteHeightPx: DEFAULT_PLAYFIELD_PARAMS.noteHeightPx,
  fallbackTailHeightPx: DEFAULT_PLAYFIELD_PARAMS.lnTailHeightPx,
  columnColour: { r: 0, g: 0, b: 0, a: 255 },
  columnLineColour: { r: 255, g: 255, b: 255, a: 255 },
  judgementLineColour: { r: 255, g: 255, b: 255, a: 255 },
};

export interface SkinRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface SkinFill {
  style: string;
  alpha: number;
}

export interface SkinColumn {
  x: number;
  w: number;
  noteH: number;
  headH: number;
  tailH: number;
  key: SkinRect | null;
  background: SkinFill;
}

export interface SkinLayout {
  /** Canvas px per unit of stable's 480-high space, zoom included. */
  k: number;
  /** Canvas px per lazer unit (768-high space). */
  unit: number;
  height: number;
  /** Stage plus the stage-left and stage-right sprites. */
  width: number;
  stageX: number;
  stageWidth: number;
  judgeY: number;
  columns: SkinColumn[];
  columnLines: { x: number; w: number }[];
  columnLineBottom: number;
  columnLine: SkinFill;
  judgementLine: (SkinRect & { fill: SkinFill }) | null;
  hitTarget: SkinRect | null;
  stageLeft: SkinRect | null;
  stageRight: SkinRect | null;
  stageBottom: SkinRect | null;
}

export type SkinNoteKind = "tap" | "lnHead" | "lnTail";

/** lazer's chains: head → note, tail → head → note (`LegacyHoldNoteHeadPiece.cs`, `LegacyHoldNoteTailPiece.cs`). */
export function noteImage(skin: Pick<LoadedSkin, "images">, col: number, kind: SkinNoteKind): SkinImage | undefined {
  const note = skin.images.get(SKIN_SLOT.note(col));
  if (kind === "tap") {
    return note;
  }
  const head = skin.images.get(SKIN_SLOT.head(col)) ?? note;
  return kind === "lnHead" ? head : (skin.images.get(SKIN_SLOT.tail(col)) ?? head);
}

export function displayWidth(img: SkinImage): number {
  return img.width / img.scale;
}

export function displayHeight(img: SkinImage): number {
  return img.height / img.scale;
}

/**
 * A percy-style LN body: cropped by the loader, or at least `stripBodyAspect` times taller than wide. Its art is drawn
 * once from the tail, never tiled (research 06).
 */
export function isStripBody(img: SkinImage, params: SkinLayoutParams = DEFAULT_SKIN_LAYOUT_PARAMS): boolean {
  return img.sourceHeight > img.height || displayHeight(img) >= params.stripBodyAspect * displayWidth(img);
}

/** Column backgrounds and lines take the alpha twice, so alpha 0 hides them (`LegacyColourCompatibility.cs` L38–42). */
export function doubledAlpha(c: SkinRgba): number {
  return (c.a / 255) ** 2;
}

/** Colours set after construction in stable never stay fully transparent (`LegacyColourCompatibility.cs` L22–26). */
export function opaqueAlpha(c: SkinRgba): number {
  return c.a === 0 ? 1 : c.a / 255;
}

function rgb(c: SkinRgba): string {
  return `rgb(${c.r}, ${c.g}, ${c.b})`;
}

export function skinLayout(
  skin: LoadedSkin,
  height: number,
  zoom: number,
  params: SkinLayoutParams = DEFAULT_SKIN_LAYOUT_PARAMS,
  stageParams: StageParams = DEFAULT_STAGE_PARAMS,
): SkinLayout {
  const k = (height / stageParams.virtualHeightPx) * clampZoom(zoom, stageParams);
  const unit = k / params.stableMagicScale;
  // Zoom widens the stage only; the judgement line stays where HitPosition puts it, as in the procedural stage.
  const judgeY = judgeYFromHitPosition(skin.hitPosition, height, stageParams);

  const left = skin.images.get(SKIN_SLOT.stageLeft);
  const right = skin.images.get(SKIN_SLOT.stageRight);
  const stageX = left === undefined ? 0 : displayWidth(left) * unit;

  const widths = skin.columnWidth.map((w) => w * k);
  const minWidth = Math.min(...skin.columnWidth);
  const scaleWidth = skin.widthForNoteHeightScale;
  // lazer treats a non-positive value as unset (research 06); 0 would hide every note.
  const noteScaleWidth = (scaleWidth !== null && scaleWidth > 0 ? scaleWidth : minWidth) * k;
  const spriteHeight = (img: SkinImage | undefined, fallback: number): number =>
    img === undefined ? fallback : (displayHeight(img) * noteScaleWidth) / displayWidth(img);

  const columns: SkinColumn[] = [];
  let x = stageX;
  for (const [i, w] of widths.entries()) {
    if (i > 0) {
      x += (skin.columnSpacing[i - 1] ?? 0) * k;
    }
    const keyImg = skin.images.get(SKIN_SLOT.key(i));
    const keyH = keyImg === undefined ? 0 : displayHeight(keyImg) * k * params.keyHeightPerK;
    const colour = skin.colours.column[i] ?? params.columnColour;
    columns.push({
      x,
      w,
      noteH: spriteHeight(noteImage(skin, i, "tap"), params.fallbackNoteHeightPx),
      headH: spriteHeight(noteImage(skin, i, "lnHead"), params.fallbackNoteHeightPx),
      tailH: spriteHeight(noteImage(skin, i, "lnTail"), params.fallbackTailHeightPx),
      key: keyImg === undefined ? null : { x, y: height - keyH, w, h: keyH },
      background: { style: rgb(colour), alpha: doubledAlpha(colour) },
    });
    x += w;
  }
  const stageWidth = x - stageX;

  const columnLines: { x: number; w: number }[] = [];
  const rightLines = skin.version >= params.rightLinesFromVersion;
  for (const [i, column] of columns.entries()) {
    const isLast = i === columns.length - 1;
    const leftW = skin.columnLineWidth[i] ?? 0;
    const rightW = skin.columnLineWidth[i + 1] ?? 0;
    if (leftW > 0) {
      columnLines.push({ x: column.x, w: leftW * params.columnLineScaleX * unit });
    }
    if (rightW > 0 && (rightLines || isLast)) {
      const offset = isLast ? params.lastColumnLineOffsetUnits * unit : 0;
      columnLines.push({ x: column.x + column.w + offset, w: rightW * params.columnLineScaleX * unit });
    }
  }
  const lineColour = skin.colours.columnLine ?? params.columnLineColour;

  const judgementColour = skin.colours.judgementLine ?? params.judgementLineColour;
  const hint = skin.images.get(SKIN_SLOT.stageHint);
  const hintH = hint === undefined ? 0 : displayHeight(hint) * params.hitTargetScaleY * unit;
  const bottom = skin.images.get(SKIN_SLOT.stageBottom);
  const bottomScale = (params.stageBottomScale / params.stableMagicScale) * k;
  const bottomW = bottom === undefined ? 0 : displayWidth(bottom) * bottomScale;
  const bottomH = bottom === undefined ? 0 : displayHeight(bottom) * bottomScale;

  return {
    k,
    unit,
    height,
    width: stageX + stageWidth + (right === undefined ? 0 : displayWidth(right) * unit),
    stageX,
    stageWidth,
    judgeY,
    columns,
    columnLines,
    columnLineBottom: judgeY,
    columnLine: { style: rgb(lineColour), alpha: doubledAlpha(lineColour) },
    judgementLine: skin.judgementLine
      ? {
          x: stageX,
          y: judgeY,
          w: stageWidth,
          h: params.judgementLineUnits * unit,
          fill: { style: rgb(judgementColour), alpha: params.judgementLineAlpha * opaqueAlpha(judgementColour) },
        }
      : null,
    hitTarget: hint === undefined ? null : { x: stageX, y: judgeY - hintH / 2, w: stageWidth, h: hintH },
    stageLeft: left === undefined ? null : { x: 0, y: 0, w: stageX, h: height },
    stageRight:
      right === undefined ? null : { x: stageX + stageWidth, y: 0, w: displayWidth(right) * unit, h: height },
    stageBottom:
      bottom === undefined
        ? null
        : { x: stageX + stageWidth / 2 - bottomW / 2, y: height - bottomH, w: bottomW, h: bottomH },
  };
}
