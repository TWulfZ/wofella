import { GripVertical } from "lucide-react";
import { type KeyboardEvent, type PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { readPanelWidthPx, writePanelWidthPx } from "../prefs";

export interface PanelWidthParams {
  minPx: number;
  defaultPx: number;
  /** Share of the viewport the panel may take, so the playfield never vanishes. */
  maxViewportFraction: number;
  stepPx: number;
  largeStepPx: number;
}

export const PANEL_WIDTH_PARAMS: PanelWidthParams = {
  minPx: 320,
  defaultPx: 416,
  maxViewportFraction: 0.7,
  stepPx: 16,
  largeStepPx: 64,
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

export interface PanelWidth {
  width: number;
  min: number;
  max: number;
  dragging: boolean;
  /** `persist` is false while dragging, so a drag writes the storage once, when it ends. */
  setWidth: (width: number, persist: boolean) => number;
  setDragging: (dragging: boolean) => void;
}

export function usePanelWidth(params: PanelWidthParams = PANEL_WIDTH_PARAMS): PanelWidth {
  const [stored, setStored] = useState(() => readPanelWidthPx(params.defaultPx));
  const [viewport, setViewport] = useState(() => window.innerWidth);
  const [dragging, setDragging] = useState(false);
  useEffect(() => {
    const onResize = (): void => {
      setViewport(window.innerWidth);
    };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
    };
  }, []);
  const min = params.minPx;
  const max = Math.max(min, Math.floor(viewport * params.maxViewportFraction));
  return {
    width: clamp(stored, min, max),
    min,
    max,
    dragging,
    setWidth: (next, persist) => {
      const clamped = clamp(Math.round(next), min, max);
      setStored(clamped);
      if (persist) {
        writePanelWidthPx(clamped);
      }
      return clamped;
    },
    setDragging,
  };
}

interface PanelResizerProps {
  panel: PanelWidth;
  params?: PanelWidthParams;
}

/** The panel sits right of the handle, so moving the handle left widens it. */
export function PanelResizer({ panel, params = PANEL_WIDTH_PARAMS }: PanelResizerProps) {
  const { t } = useTranslation();
  const { width, min, max, setWidth, setDragging } = panel;
  const cleanup = useRef<(() => void) | null>(null);
  useEffect(
    () => () => {
      cleanup.current?.();
    },
    [],
  );

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>): void => {
    const step = e.shiftKey ? params.largeStepPx : params.stepPx;
    const next =
      e.key === "ArrowLeft"
        ? width + step
        : e.key === "ArrowRight"
          ? width - step
          : e.key === "Home"
            ? min
            : e.key === "End"
              ? max
              : null;
    if (next === null) {
      return;
    }
    e.preventDefault();
    setWidth(next, true);
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>): void => {
    if (e.button !== 0) {
      return;
    }
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = width;
    let last = width;
    // Window listeners, not pointer capture: the pointer may leave the 8px handle mid-drag and still has to steer it.
    const onMove = (ev: PointerEvent): void => {
      last = setWidth(startWidth + startX - ev.clientX, false);
    };
    const stop = (): void => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      cleanup.current = null;
      setDragging(false);
    };
    const onUp = (): void => {
      stop();
      setWidth(last, true);
    };
    cleanup.current?.();
    cleanup.current = stop;
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    setDragging(true);
  };

  return (
    <div
      role="separator"
      tabIndex={0}
      aria-orientation="vertical"
      aria-label={t("label.panel.resize")}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={width}
      aria-valuetext={t("label.panel.widthValue", { px: width })}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      className={cn(
        "group relative flex w-2 shrink-0 cursor-col-resize touch-none items-center justify-center rounded-full outline-none",
        "focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-2",
      )}
    >
      <span
        aria-hidden
        className={cn(
          "bg-border h-full w-px motion-safe:transition-colors motion-safe:duration-150",
          "group-hover:bg-primary/70 group-focus-visible:bg-primary",
          panel.dragging && "bg-primary",
        )}
      />
      <GripVertical
        aria-hidden
        className={cn(
          "text-muted-foreground bg-background absolute size-4 rounded-sm",
          "group-hover:text-primary group-focus-visible:text-primary",
          panel.dragging && "text-primary",
        )}
      />
    </div>
  );
}
