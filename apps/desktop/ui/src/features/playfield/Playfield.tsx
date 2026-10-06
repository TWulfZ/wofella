import { useEffect, useRef, useState } from "react";
import { cn } from "@/shared/lib/utils";
import type { Clock } from "./audioClock";
import { DEFAULT_PLAYFIELD_THEME, draw } from "./draw";
import { fitPxPerMs, project } from "./project";
import {
  DEFAULT_STAGE_PARAMS,
  judgeYFromHitPosition,
  type ScrollMode,
  scrollPxPerMs,
  stageWidthPx,
  uniformColumnWidths,
} from "./stage";
import type { ChartWindow } from "./types";

export interface PlayfieldProps {
  window: ChartWindow;
  clock: Clock | null;
  /** "fit" scales the whole window above the judgement line; any other mode is used paused and playing alike. */
  scroll: ScrollMode | "fit";
  /** In stable's 480-px space (skin.ini HitPosition). */
  hitPosition?: number;
  /** In stable's 480-px space (skin.ini ColumnWidth); one per column. */
  columnWidths?: readonly number[];
  zoom?: number;
  rate?: number;
  className?: string;
}

export function Playfield(props: PlayfieldProps) {
  const { window: chartWindow, clock, scroll, className } = props;
  const {
    hitPosition = DEFAULT_STAGE_PARAMS.defaultHitPosition,
    columnWidths,
    zoom = DEFAULT_STAGE_PARAMS.defaultZoom,
    rate = 1,
  } = props;
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });

  useEffect(() => {
    const container = containerRef.current;
    if (container === null) {
      return;
    }
    const observer = new ResizeObserver((entries) => {
      const rect = entries.at(-1)?.contentRect;
      if (rect !== undefined) {
        setSize({ width: rect.width, height: rect.height });
      }
    });
    observer.observe(container);
    return () => {
      observer.disconnect();
    };
  }, []);

  const { height } = size;
  const stageWidth = stageWidthPx(columnWidths ?? uniformColumnWidths(chartWindow.layout.columns.length), height, zoom);
  const width = Math.min(stageWidth, size.width);
  const left = (size.width - width) / 2;
  const judgeY = judgeYFromHitPosition(hitPosition, height);
  const pxPerMs = scroll === "fit" ? fitPxPerMs(chartWindow, height, judgeY) : scrollPxPerMs(scroll, height, rate);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null || width <= 0 || height <= 0) {
      return;
    }
    const ctx = canvas.getContext("2d");
    if (ctx === null) {
      return;
    }
    const dpr = globalThis.devicePixelRatio > 0 ? globalThis.devicePixelRatio : 1;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const view = { width, height, judgeY };
    const render = (nowMs: number): void => {
      draw(ctx, project(chartWindow, { ...view, nowMs, pxPerMs }), DEFAULT_PLAYFIELD_THEME, view);
    };
    // Paused at the chosen speed, so pressing play does not rescale what was just read.
    const renderStatic = (): void => {
      render(chartWindow.fromMs);
    };

    if (clock === null) {
      renderStatic();
      return;
    }
    // Clock exposes no change events, so play/pause is picked up by polling `playing` each frame.
    let staticDrawn = false;
    let frame = 0;
    const tick = (): void => {
      if (clock.playing) {
        render(clock.nowMs());
        staticDrawn = false;
      } else if (!staticDrawn) {
        renderStatic();
        staticDrawn = true;
      }
      frame = requestAnimationFrame(tick);
    };
    tick();
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [chartWindow, clock, pxPerMs, judgeY, width, height]);

  return (
    <div ref={containerRef} className={cn("relative overflow-hidden", className)}>
      <canvas
        ref={canvasRef}
        className="absolute top-0 block h-full rounded-md"
        style={{ width: `${width}px`, left: `${left}px` }}
      />
    </div>
  );
}
