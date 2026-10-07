import { columnColor, PLAYFIELD_COLORS } from "./colors";
import { drawSkinEffects, drawStageLights, drawTextEffects, keySprite, type PlayfieldFx } from "./drawEffects";
import type { NoteSpan, Projection, ProjectView } from "./project";
import {
  DEFAULT_SKIN_LAYOUT_PARAMS,
  displayHeight,
  displayWidth,
  isStripBody,
  noteImage,
  type SkinFill,
  type SkinLayout,
  type SkinLayoutParams,
  type SkinRect,
} from "./skinLayout";
import { type LoadedSkin, type NoteBodyStyle, SKIN_SLOT, type SkinImage } from "./skinModel";
import type { StageBackground } from "./stageLayers";

export interface PlayfieldTheme {
  background: string;
  columnTint: string;
  columnSeparator: string;
  beatLine: string;
  measureLine: string;
  handSeparator: string;
  judgementLine: string;
  shade: string;
  maxText: string;
  comboText: string;
  lnBodyAlpha: number;
  columnColor: (keymode: number, col: number) => string;
  columnSeparatorPx: number;
  handSeparatorPx: number;
  beatLinePx: number;
  measureLinePx: number;
  judgementLinePx: number;
  /** Horizontal inset on each side of a note, so adjacent columns read apart. */
  noteGapPx: number;
}

export const DEFAULT_PLAYFIELD_THEME: PlayfieldTheme = {
  ...PLAYFIELD_COLORS,
  columnColor,
  columnSeparatorPx: 1,
  handSeparatorPx: 3,
  beatLinePx: 1,
  measureLinePx: 2,
  judgementLinePx: 3,
  noteGapPx: 1,
};

/** The slice of CanvasRenderingContext2D the playfield draws with: filled rects, skin images and effect text. */
export interface Draw2D {
  fillStyle: string | CanvasGradient | CanvasPattern;
  globalAlpha: number;
  globalCompositeOperation: GlobalCompositeOperation;
  font: string;
  textAlign: CanvasTextAlign;
  textBaseline: CanvasTextBaseline;
  fillRect(x: number, y: number, w: number, h: number): void;
  fillText(text: string, x: number, y: number): void;
  save(): void;
  restore(): void;
  drawImage(
    image: CanvasImageSource,
    sx: number,
    sy: number,
    sw: number,
    sh: number,
    dx: number,
    dy: number,
    dw: number,
    dh: number,
  ): void;
  translate(x: number, y: number): void;
  scale(x: number, y: number): void;
}

export type DrawView = Pick<ProjectView, "width" | "height" | "judgeY">;

/** Draws in CSS px; the caller owns the device-pixel transform. */
export function draw(ctx: Draw2D, projection: Projection, theme: PlayfieldTheme, view: DrawView, fx?: PlayfieldFx): void {
  const { width, height, judgeY } = view;
  const fill = (style: string, x: number, y: number, w: number, h: number): void => {
    ctx.fillStyle = style;
    ctx.fillRect(x, y, w, h);
  };
  const hLine = (style: string, y: number, px: number): void => {
    fill(style, 0, y - px / 2, width, px);
  };
  const vLine = (style: string, x: number, px: number): void => {
    fill(style, x - px / 2, 0, px, height);
  };

  ctx.globalAlpha = 1;
  fill(theme.background, 0, 0, width, height);
  for (const column of projection.columns) {
    fill(theme.columnTint, column.x, 0, column.w, height);
  }
  for (const line of projection.beatLines) {
    if (line.measure) {
      hLine(theme.measureLine, line.y, theme.measureLinePx);
    } else {
      hLine(theme.beatLine, line.y, theme.beatLinePx);
    }
  }
  const handXs = new Set(projection.handSeparators.map((s) => s.x));
  for (const column of projection.columns.slice(1)) {
    if (!handXs.has(column.x)) {
      vLine(theme.columnSeparator, column.x, theme.columnSeparatorPx);
    }
  }
  for (const separator of projection.handSeparators) {
    vLine(theme.handSeparator, separator.x, theme.handSeparatorPx);
  }

  const keymode = projection.columns.length;
  const gap = theme.noteGapPx;
  for (const note of projection.notes) {
    ctx.globalAlpha = note.kind === "lnBody" ? theme.lnBodyAlpha : 1;
    fill(theme.columnColor(keymode, note.col), note.x + gap, note.y, note.w - 2 * gap, note.h);
  }
  ctx.globalAlpha = 1;

  for (const region of projection.shade) {
    fill(theme.shade, 0, region.y0, width, region.y1 - region.y0);
  }
  hLine(theme.judgementLine, judgeY, theme.judgementLinePx);
  drawTextEffects(ctx, view, theme, fx);
}

/**
 * Draws the chart with a legacy skin, after lazer's legacy pieces (research 06, "Geometry"). Each element whose slot
 * the skin does not resolve is drawn as the procedural playfield draws it, one slot at a time (ADR 0019). With
 * `stage`, the opaque background is painted once offscreen and blitted every frame. `fx` adds the preview's toggles
 * and autoplay effects; without it the picture is the same as with `DEFAULT_PLAYFIELD_EFFECTS`.
 */
export function drawSkinned(
  ctx: Draw2D,
  projection: Projection,
  layout: SkinLayout,
  skin: LoadedSkin,
  theme: PlayfieldTheme,
  view: DrawView,
  params: SkinLayoutParams = DEFAULT_SKIN_LAYOUT_PARAMS,
  stage?: { background: StageBackground; dpr: number },
  fx?: PlayfieldFx,
): void {
  const { width, height } = view;
  const { stageX, stageWidth } = layout;
  const on = painter(ctx, params);
  const { fill, image } = on;
  const skinFill = (f: SkinFill, x: number, y: number, w: number, h: number): void => {
    fill(f.style, f.alpha, x, y, w, h);
  };
  const slotImage = (img: SkinImage | undefined, r: SkinRect | null): void => {
    if (img !== undefined && r !== null) {
      image(img, r);
    }
  };
  const keys = (): void => {
    for (const i of layout.columns.keys()) {
      const key = keySprite(skin, layout, i, fx, params);
      if (key !== null) {
        image(key.img, key.rect);
      }
    }
  };

  const scene: StageScene = { layout, skin, theme, width, height };
  const cached =
    stage?.background.get([layout, skin, theme, params], { width, height, dpr: stage.dpr }, (target) => {
      paintBackground(painter(target, params), scene);
    }) ?? null;
  if (cached === null) {
    paintBackground(on, scene);
  } else {
    ctx.globalAlpha = 1;
    ctx.drawImage(cached.image, 0, 0, cached.pxWidth, cached.pxHeight, 0, 0, width, height);
  }

  for (const line of projection.beatLines) {
    const px = line.measure ? theme.measureLinePx : theme.beatLinePx;
    fill(line.measure ? theme.measureLine : theme.beatLine, 1, stageX, line.y - px / 2, stageWidth, px);
  }
  for (const line of layout.columnLines) {
    skinFill(layout.columnLine, line.x, 0, line.w, layout.columnLineBottom);
  }
  for (const [i, column] of projection.columns.entries()) {
    const prev = projection.columns[i - 1];
    const left = layout.columns[i - 1];
    const right = layout.columns[i];
    if (prev !== undefined && prev.hand !== column.hand && left !== undefined && right !== undefined) {
      const x = (left.x + left.w + right.x) / 2;
      fill(theme.handSeparator, 1, x - theme.handSeparatorPx / 2, 0, theme.handSeparatorPx, height);
    }
  }
  slotImage(skin.images.get(SKIN_SLOT.stageHint), layout.hitTarget);
  if (layout.judgementLine !== null) {
    const { fill: f, x, y, w, h } = layout.judgementLine;
    skinFill(f, x, y, w, h);
  }
  drawStageLights(ctx, layout, skin, fx);
  if (skin.keysUnderNotes) {
    keys();
  }
  const keymode = layout.columns.length;
  const plainPercy = fx?.flags.percy === false;
  for (const span of projection.spans) {
    drawSpan(ctx, span, { layout, skin, theme, params, keymode, height, fill, image, plainPercy });
  }
  if (!skin.keysUnderNotes) {
    keys();
  }
  slotImage(skin.images.get(SKIN_SLOT.stageBottom), layout.stageBottom);

  for (const region of projection.shade) {
    fill(theme.shade, 1, 0, region.y0, width, region.y1 - region.y0);
  }
  // The skin's hit target marks the judgement line; without one the procedural line stands in for it.
  if (!skin.images.has(SKIN_SLOT.stageHint)) {
    fill(theme.judgementLine, 1, stageX, view.judgeY - theme.judgementLinePx / 2, stageWidth, theme.judgementLinePx);
  }
  ctx.globalAlpha = 1;
  drawSkinEffects(ctx, layout, skin, theme, fx);
}

interface Painter {
  fill: Fill;
  image: (img: SkinImage, r: SkinRect) => void;
}

function painter(ctx: Draw2D, params: SkinLayoutParams): Painter {
  return {
    fill: (style, alpha, x, y, w, h) => {
      ctx.globalAlpha = alpha;
      ctx.fillStyle = style;
      ctx.fillRect(x, y, w, h);
    },
    image: (img, r) => {
      if (r.h < params.minImageDrawPx) {
        return;
      }
      ctx.globalAlpha = 1;
      ctx.drawImage(img.bitmap, 0, 0, img.width, img.height, r.x, r.y, r.w, r.h);
    },
  };
}

interface StageScene {
  layout: SkinLayout;
  skin: LoadedSkin;
  theme: PlayfieldTheme;
  width: number;
  height: number;
}

function paintBackground(on: Painter, scene: StageScene): void {
  const { layout, skin, theme, width, height } = scene;
  on.fill(theme.background, 1, 0, 0, width, height);
  for (const [slot, r] of [
    [SKIN_SLOT.stageLeft, layout.stageLeft],
    [SKIN_SLOT.stageRight, layout.stageRight],
  ] as const) {
    const img = skin.images.get(slot);
    if (img !== undefined && r !== null) {
      on.image(img, r);
    }
  }
  for (const column of layout.columns) {
    on.fill(column.background.style, column.background.alpha, column.x, 0, column.w, height);
  }
}

type Fill = (style: string, alpha: number, x: number, y: number, w: number, h: number) => void;

interface SpanContext {
  layout: SkinLayout;
  skin: LoadedSkin;
  theme: PlayfieldTheme;
  params: SkinLayoutParams;
  keymode: number;
  height: number;
  fill: Fill;
  image: (img: SkinImage, r: SkinRect) => void;
  /** Percy switched off: percy bodies give way to a plain bar and a visible tail cap, for reading. */
  plainPercy: boolean;
}

function drawSpan(ctx: Draw2D, span: NoteSpan, sc: SpanContext): void {
  const { layout, skin, theme, keymode, height, fill, image } = sc;
  const column = layout.columns[span.col];
  if (column === undefined) {
    return;
  }
  const { x, w } = column;
  const colour = theme.columnColor(keymode, span.col);
  const gap = theme.noteGapPx;
  const sprite = (img: SkinImage | undefined, bottom: number, h: number): void => {
    if (bottom - h >= height) {
      return;
    }
    if (img === undefined) {
      fill(colour, 1, x + gap, bottom - h, w - 2 * gap, h);
    } else {
      image(img, { x, y: bottom - h, w, h });
    }
  };

  if (span.tailY === null) {
    sprite(noteImage(skin, span.col, "tap"), span.headY, column.noteH);
    return;
  }
  const top = span.tailY;
  const bottom = span.headY;
  const body = skin.images.get(SKIN_SLOT.body(span.col));
  if (sc.plainPercy && body !== undefined && isStripBody(body, sc.params)) {
    // Percy's art hides the tail behind a transparent lead-in; the procedural LN shows where the hold ends.
    if (bottom > top && top < height) {
      const y0 = Math.max(top, 0);
      fill(colour, theme.lnBodyAlpha, x + gap, y0, w - 2 * gap, Math.min(bottom, height) - y0);
    }
    const capH = sc.params.fallbackTailHeightPx;
    if (top > 0 && top - capH < height) {
      fill(colour, 1, x + gap, top - capH, w - 2 * gap, capH);
    }
    sprite(noteImage(skin, span.col, "lnHead"), bottom, column.headH);
    return;
  }
  if (bottom > top && top < height) {
    if (body === undefined) {
      const y0 = Math.max(top, 0);
      fill(colour, theme.lnBodyAlpha, x + gap, y0, w - 2 * gap, Math.min(bottom, height) - y0);
    } else {
      drawBody(ctx, body, skin.noteBodyStyle, { x, y: top, w, h: bottom - top }, height, sc.params);
    }
  }

  sprite(skin.lnTails.get(span.col), top, column.tailH);
  sprite(noteImage(skin, span.col, "lnHead"), bottom, column.headH);
}

/**
 * Every repeat style puts the image's row 0 at the tail and tiles toward the head at the column's width-fit height,
 * as lazer does (`LegacyBodyPiece.cs`, TopCentre) and percy skins rely on. Whether stable keeps that natural aspect
 * or stretches like lazer (32800 units) is unverified (research 06).
 */
function drawBody(
  ctx: Draw2D,
  img: SkinImage,
  style: NoteBodyStyle,
  body: SkinRect,
  height: number,
  params: SkinLayoutParams,
): void {
  const tileH = (displayHeight(img) * body.w) / displayWidth(img);
  const cropped = img.sourceHeight > img.height;
  ctx.globalAlpha = 1;
  if ((style === "Stretch" && !cropped) || !(tileH >= params.minBodyTilePx)) {
    ctx.drawImage(img.bitmap, 0, 0, img.width, img.height, body.x, body.y, body.w, body.h);
    return;
  }
  const visTop = Math.max(body.y, 0);
  const visBottom = Math.min(body.y + body.h, height);
  const slice = (t0: number, t1: number, sy: number, sh: number): void => {
    const y0 = Math.max(t0, visTop);
    const y1 = Math.min(t1, visBottom);
    if (y1 <= y0) {
      return;
    }
    const scale = sh / (t1 - t0);
    ctx.drawImage(img.bitmap, 0, sy + (y0 - t0) * scale, img.width, (y1 - y0) * scale, body.x, y0, body.w, y1 - y0);
  };
  const strip = (artH: number): void => {
    const artEnd = body.y + artH;
    slice(body.y, artEnd, 0, img.height);
    const rows = Math.min(params.stripTailRows, img.height);
    // A strip ends in a plain bar, so its last rows can be stretched onto just the visible rest of the hold.
    const restTop = Math.max(artEnd, visTop);
    if (body.y + body.h > artEnd && visBottom > restTop) {
      ctx.drawImage(img.bitmap, 0, img.height - rows, img.width, rows, body.x, restTop, body.w, visBottom - restTop);
    }
  };

  if (style === "Stretch") {
    // The whole file spans the hold; the crop kept only its top rows, so they cover their share of it (ADR 0019 cap).
    strip((body.h * img.height) / img.sourceHeight);
    return;
  }
  if (isStripBody(img, params)) {
    strip(tileH);
    return;
  }
  for (let n = Math.max(0, Math.floor((visTop - body.y) / tileH)); ; n++) {
    const t0 = body.y + n * tileH;
    if (t0 >= visBottom) {
      break;
    }
    slice(t0, t0 + tileH, 0, img.height);
  }
}
