import { useEffect, useRef } from "react";
import { cn } from "@/shared/lib/utils";
import { DEFAULT_PLAYFIELD_THEME, draw, type PlayfieldTheme } from "./draw";
import { DEFAULT_PLAYFIELD_PARAMS, fitPxPerMs, type PlayfieldParams, project } from "./project";
import type { ChartWindow } from "./types";

export interface PatternPreviewParams {
  /** Note height as a fraction of the preview height, clamped to the px range below. */
  noteHeightRatio: number;
  minNoteHeightPx: number;
  maxNoteHeightPx: number;
  /** Room under the judgement line, so the line itself stays inside the canvas. */
  judgeMarginPx: number;
}

export const PATTERN_PREVIEW_PARAMS: PatternPreviewParams = {
  noteHeightRatio: 1 / 48,
  minNoteHeightPx: 3,
  maxNoteHeightPx: 5,
  judgeMarginPx: 2,
};

// Thumbnails are a few px per column, so full-size separators would bury the notes.
const PREVIEW_THEME: PlayfieldTheme = {
  ...DEFAULT_PLAYFIELD_THEME,
  columnSeparatorPx: 0.5,
  handSeparatorPx: 1.5,
  beatLinePx: 0.5,
  measureLinePx: 1,
  judgementLinePx: 1.5,
};

function previewParams(height: number, params: PatternPreviewParams): PlayfieldParams {
  const noteHeightPx = Math.min(
    Math.max(Math.round(height * params.noteHeightRatio), params.minNoteHeightPx),
    params.maxNoteHeightPx,
  );
  return { ...DEFAULT_PLAYFIELD_PARAMS, noteHeightPx, lnTailHeightPx: Math.max(1, Math.round(noteHeightPx / 2)) };
}

export interface PatternPreviewProps {
  window: ChartWindow;
  /** Drawing size in CSS px; the element's displayed width comes from `className` and keeps this aspect ratio. */
  width: number;
  height: number;
  label: string;
  className?: string;
}

/** The whole window paused at `fromMs`, drawn once per change: no clock, no resize tracking. */
export function PatternPreview({ window: chartWindow, width, height, label, className }: PatternPreviewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d") ?? null;
    if (canvas === null || ctx === null || width <= 0 || height <= 0) {
      return;
    }
    const dpr = globalThis.devicePixelRatio > 0 ? globalThis.devicePixelRatio : 1;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const params = previewParams(height, PATTERN_PREVIEW_PARAMS);
    const view = { width, height, judgeY: height - PATTERN_PREVIEW_PARAMS.judgeMarginPx };
    const pxPerMs = fitPxPerMs(chartWindow, height, view.judgeY, params);
    draw(ctx, project(chartWindow, { ...view, nowMs: chartWindow.fromMs, pxPerMs }, params), PREVIEW_THEME, view);
  }, [chartWindow, width, height]);

  return (
    <canvas
      ref={canvasRef}
      role="img"
      aria-label={label}
      className={cn("block h-auto w-full rounded-md", className)}
      style={{ aspectRatio: `${width} / ${height}` }}
    />
  );
}
