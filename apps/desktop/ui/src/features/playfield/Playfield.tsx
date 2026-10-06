import { useEffect, useRef, useState } from "react";
import { cn } from "@/shared/lib/utils";
import type { Clock } from "./audioClock";
import { DEFAULT_PLAYFIELD_THEME, draw } from "./draw";
import { fitPxPerMs, project } from "./project";
import type { ChartWindow } from "./types";

export interface PlayfieldViewParams {
  /** Judgement line distance from the bottom edge when the caller does not place it. */
  judgeInsetPx: number;
}

export const DEFAULT_PLAYFIELD_VIEW: PlayfieldViewParams = { judgeInsetPx: 64 };

export interface PlayfieldProps {
  window: ChartWindow;
  clock: Clock | null;
  /** Scroll speed while playing, in px per ms; the static view always fits the whole window. */
  scroll: number | "fit";
  /** In CSS px from the top. */
  judgeY?: number;
  className?: string;
}

export function Playfield({ window: chartWindow, clock, scroll, judgeY, className }: PlayfieldProps) {
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

  useEffect(() => {
    const canvas = canvasRef.current;
    const { width, height } = size;
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

    const view = { width, height, judgeY: judgeY ?? Math.max(height - DEFAULT_PLAYFIELD_VIEW.judgeInsetPx, 0) };
    const fit = fitPxPerMs(chartWindow, height, view.judgeY);
    const render = (nowMs: number, pxPerMs: number): void => {
      draw(ctx, project(chartWindow, { ...view, nowMs, pxPerMs }), DEFAULT_PLAYFIELD_THEME, view);
    };
    const renderStatic = (): void => {
      render(chartWindow.fromMs, fit);
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
        render(clock.nowMs(), scroll === "fit" ? fit : scroll);
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
  }, [chartWindow, clock, scroll, judgeY, size]);

  return (
    <div ref={containerRef} className={cn("relative overflow-hidden", className)}>
      <canvas ref={canvasRef} className="absolute inset-0 block h-full w-full" />
    </div>
  );
}
