import { columnColor, PLAYFIELD_COLORS } from "./colors";
import type { Projection, ProjectView } from "./project";

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

/** The slice of CanvasRenderingContext2D the playfield draws with; every shape is a filled rect. */
export interface Draw2D {
  fillStyle: string | CanvasGradient | CanvasPattern;
  globalAlpha: number;
  fillRect(x: number, y: number, w: number, h: number): void;
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
