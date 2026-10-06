import { columnColor, PLAYFIELD_COLORS } from "./colors";
import type { NoteSpan, Projection, ProjectView } from "./project";
import {
  DEFAULT_SKIN_LAYOUT_PARAMS,
  displayHeight,
  displayWidth,
  noteImage,
  type SkinFill,
  type SkinLayout,
  type SkinLayoutParams,
  type SkinRect,
} from "./skinLayout";
import { type LoadedSkin, type NoteBodyStyle, SKIN_SLOT, type SkinImage } from "./skinModel";

export interface PlayfieldTheme {
  background: string;
  columnTint: string;
  columnSeparator: string;
  beatLine: string;
  measureLine: string;
  handSeparator: string;
  judgementLine: string;
  shade: string;
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

/** The slice of CanvasRenderingContext2D the playfield draws with: filled rects, and skin images. */
export interface Draw2D {
  fillStyle: string | CanvasGradient | CanvasPattern;
  globalAlpha: number;
  fillRect(x: number, y: number, w: number, h: number): void;
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
  save(): void;
  restore(): void;
  translate(x: number, y: number): void;
  scale(x: number, y: number): void;
}

export type DrawView = Pick<ProjectView, "width" | "height" | "judgeY">;

/** Draws in CSS px; the caller owns the device-pixel transform. */
export function draw(ctx: Draw2D, projection: Projection, theme: PlayfieldTheme, view: DrawView): void {
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
}

/**
 * Draws the chart with a legacy skin, after lazer's legacy pieces (research 06, "Geometry"). Each element whose slot
 * the skin does not resolve is drawn as the procedural playfield draws it, one slot at a time (ADR 0019).
 */
export function drawSkinned(
  ctx: Draw2D,
  projection: Projection,
  layout: SkinLayout,
  skin: LoadedSkin,
  theme: PlayfieldTheme,
  view: DrawView,
  params: SkinLayoutParams = DEFAULT_SKIN_LAYOUT_PARAMS,
): void {
  const { width, height } = view;
  const { stageX, stageWidth } = layout;
  const fill = (style: string, alpha: number, x: number, y: number, w: number, h: number): void => {
    ctx.globalAlpha = alpha;
    ctx.fillStyle = style;
    ctx.fillRect(x, y, w, h);
  };
  const skinFill = (f: SkinFill, x: number, y: number, w: number, h: number): void => {
    fill(f.style, f.alpha, x, y, w, h);
  };
  const image = (img: SkinImage, r: SkinRect): void => {
    ctx.globalAlpha = 1;
    ctx.drawImage(img.bitmap, 0, 0, img.width, img.height, r.x, r.y, r.w, r.h);
  };
  const slotImage = (img: SkinImage | undefined, r: SkinRect | null): void => {
    if (img !== undefined && r !== null) {
      image(img, r);
    }
  };

  fill(theme.background, 1, 0, 0, width, height);
  slotImage(skin.images.get(SKIN_SLOT.stageLeft), layout.stageLeft);
  slotImage(skin.images.get(SKIN_SLOT.stageRight), layout.stageRight);
  for (const column of layout.columns) {
    skinFill(column.background, column.x, 0, column.w, height);
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
  const hint = skin.images.get(SKIN_SLOT.stageHint);
  slotImage(hint, layout.hitTarget);
  if (layout.judgementLine !== null) {
    const { fill: f, x, y, w, h } = layout.judgementLine;
    skinFill(f, x, y, w, h);
  }

  const keys = (): void => {
    for (const [i, column] of layout.columns.entries()) {
      slotImage(skin.images.get(SKIN_SLOT.key(i)), column.key);
    }
  };
  if (skin.keysUnderNotes) {
    keys();
  }
  const keymode = layout.columns.length;
  for (const span of projection.spans) {
    drawSpan(ctx, span, { layout, skin, theme, params, keymode, height, fill, image });
  }
  if (!skin.keysUnderNotes) {
    keys();
  }
  slotImage(skin.images.get(SKIN_SLOT.stageBottom), layout.stageBottom);

  for (const region of projection.shade) {
    fill(theme.shade, 1, 0, region.y0, width, region.y1 - region.y0);
  }
  // The skin's hit target marks the judgement line; without one the procedural line stands in for it.
  if (hint === undefined) {
    fill(theme.judgementLine, 1, stageX, view.judgeY - theme.judgementLinePx / 2, stageWidth, theme.judgementLinePx);
  }
  ctx.globalAlpha = 1;
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
  if (bottom > top && top < height) {
    if (body === undefined) {
      const y0 = Math.max(top, 0);
      fill(colour, theme.lnBodyAlpha, x + gap, y0, w - 2 * gap, Math.min(bottom, height) - y0);
    } else {
      drawBody(ctx, body, skin.noteBodyStyle, { x, y: top, w, h: bottom - top }, height, sc.params);
    }
  }

  const tail = noteImage(skin, span.col, "lnTail");
  if (tail === undefined) {
    sprite(undefined, top, column.tailH);
  } else if (top - column.tailH < height) {
    // lazer inverts the tail's scroll direction (`LegacyHoldNoteTailPiece.cs` L49): same box, image flipped vertically.
    ctx.save();
    ctx.translate(x, top);
    ctx.scale(1, -1);
    image(tail, { x: 0, y: 0, w, h: column.tailH });
    ctx.restore();
  }
  sprite(noteImage(skin, span.col, "lnHead"), bottom, column.headH);
}

/**
 * Repeat styles tile the image at the column's width-fit height, anchored at the head (bottom) or tail (top) end.
 * lazer stretches instead with a self-described guess (`LegacyBodyPiece.cs` L198–209); tiling is unverified against
 * stable (research 06). RepeatTopAndBottom has no known stable rendering, so it tiles as RepeatBottom.
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
  if (style === "Stretch" || !(tileH >= params.minBodyTilePx)) {
    ctx.globalAlpha = 1;
    ctx.drawImage(img.bitmap, 0, 0, img.width, img.height, body.x, body.y, body.w, body.h);
    return;
  }
  const visTop = Math.max(body.y, 0);
  const visBottom = Math.min(body.y + body.h, height);
  const fromTop = style === "RepeatTop";
  const tiles: { t0: number; t1: number }[] = [];
  if (fromTop) {
    for (let n = Math.max(0, Math.floor((visTop - body.y) / tileH)); ; n++) {
      const t0 = body.y + n * tileH;
      if (t0 >= visBottom) {
        break;
      }
      tiles.push({ t0, t1: t0 + tileH });
    }
  } else {
    const anchor = body.y + body.h;
    for (let n = Math.max(0, Math.floor((anchor - visBottom) / tileH)); ; n++) {
      const t1 = anchor - n * tileH;
      if (t1 <= visTop) {
        break;
      }
      tiles.push({ t0: t1 - tileH, t1 });
    }
  }
  ctx.globalAlpha = 1;
  for (const { t0, t1 } of tiles) {
    const y0 = Math.max(t0, visTop);
    const y1 = Math.min(t1, visBottom);
    if (y1 <= y0) {
      continue;
    }
    const sy = ((y0 - t0) / tileH) * img.height;
    const sh = ((y1 - y0) / tileH) * img.height;
    ctx.drawImage(img.bitmap, 0, sy, img.width, sh, body.x, y0, body.w, y1 - y0);
  }
}
