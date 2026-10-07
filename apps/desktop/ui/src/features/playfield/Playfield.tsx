import { useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/shared/lib/utils";
import type { Clock } from "./audioClock";
import { autoplayFrame, autoplayTimeline } from "./autoplay";
import { DEFAULT_PLAYFIELD_THEME, draw, drawSkinned } from "./draw";
import type { PlayfieldFx } from "./drawEffects";
import { DEFAULT_PLAYFIELD_EFFECTS, type PlayfieldEffects } from "./effects";
import { fitPxPerMs, project } from "./project";
import { skinLayout } from "./skinLayout";
import type { LoadedSkin } from "./skinModel";
import { createStageBackground } from "./stageLayers";
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
  /** Where paused or not-yet-started playback stands, polled each frame; null shows the window's start. Defaults to the clock. */
  position?: () => number | null;
  /** "fit" scales the whole window above the judgement line; any other mode is used paused and playing alike. */
  scroll: ScrollMode | "fit";
  /** In stable's 480-px space (skin.ini HitPosition). */
  hitPosition?: number;
  /** In stable's 480-px space (skin.ini ColumnWidth); one per column. */
  columnWidths?: readonly number[];
  zoom?: number;
  rate?: number;
  /** Null or absent draws procedurally; the skin's slots that did not load fall back one by one. */
  skin?: LoadedSkin | null;
  /** Skin toggles and the simulated perfect autoplay's effects; defaults to `DEFAULT_PLAYFIELD_EFFECTS`. */
  effects?: PlayfieldEffects;
  className?: string;
}

export function Playfield(props: PlayfieldProps) {
  const { window: chartWindow, clock, position, scroll, className } = props;
  const {
    hitPosition = DEFAULT_STAGE_PARAMS.defaultHitPosition,
    columnWidths,
    zoom = DEFAULT_STAGE_PARAMS.defaultZoom,
    rate = 1,
    skin = null,
    effects = DEFAULT_PLAYFIELD_EFFECTS,
  } = props;
  const { percy, judgements, combo, keyPress, lighting } = effects;
  // Keyed on the flags, not the object, so a caller rebuilding `effects` each render does not restart the loop.
  const flags = useMemo(
    () => ({ percy, judgements, combo, keyPress, lighting }),
    [percy, judgements, combo, keyPress, lighting],
  );
  const animated = judgements || combo || keyPress || lighting;
  const timeline = useMemo(() => (animated ? autoplayTimeline(chartWindow) : null), [animated, chartWindow]);
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [stageBackground] = useState(createStageBackground);

  // Released on every skin change, so a procedural playfield (skin null) holds no full-canvas surface.
  useEffect(
    () => () => {
      stageBackground.release();
    },
    [stageBackground, skin],
  );

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
  const layout = useMemo(() => (skin === null || height <= 0 ? null : skinLayout(skin, height, zoom)), [skin, height, zoom]);
  const stageWidth =
    layout?.width ??
    stageWidthPx(columnWidths ?? uniformColumnWidths(chartWindow.layout.columns.length), height, zoom);
  const width = Math.min(stageWidth, size.width);
  const left = (size.width - width) / 2;
  const judgeY = layout?.judgeY ?? judgeYFromHitPosition(hitPosition, height);
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
      const projection = project(chartWindow, { ...view, nowMs, pxPerMs });
      const fx: PlayfieldFx = { flags, frame: timeline === null ? null : autoplayFrame(timeline, nowMs) };
      if (skin === null || layout === null) {
        draw(ctx, projection, DEFAULT_PLAYFIELD_THEME, view, fx);
      } else {
        const stage = { background: stageBackground, dpr };
        drawSkinned(ctx, projection, layout, skin, DEFAULT_PLAYFIELD_THEME, view, undefined, stage, fx);
      }
    };
    // Paused at the chosen speed, so pressing play does not rescale what was just read; at the playhead, so a pause or
    // a seek shows the moment the seek bar points at.
    const pausedMs = (): number => (position === undefined ? clock?.nowMs() : position()) ?? chartWindow.fromMs;

    if (clock === null && position === undefined) {
      render(chartWindow.fromMs);
      return;
    }
    // Clock exposes no change events, so play/pause and paused seeks are picked up by polling each frame.
    let drawnMs: number | null = null;
    let frame = 0;
    const tick = (): void => {
      if (clock?.playing === true) {
        drawnMs = clock.nowMs();
        render(drawnMs);
      } else {
        const nowMs = pausedMs();
        if (nowMs !== drawnMs) {
          render(nowMs);
          drawnMs = nowMs;
        }
      }
      frame = requestAnimationFrame(tick);
    };
    tick();
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [chartWindow, clock, position, pxPerMs, judgeY, width, height, skin, layout, stageBackground, flags, timeline]);

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
