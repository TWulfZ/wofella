import {
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { formatMinSec } from "../format";
import type { Span } from "../types";

export interface ChartTimelineParams {
  stepMs: number;
  largeStepMs: number;
}

export const CHART_TIMELINE_PARAMS: ChartTimelineParams = { stepMs: 1000, largeStepMs: 5000 };

export interface ChartTimelineProps {
  /** The chart's time range; `endMs` is the furthest a window may reach. */
  span: { firstMs: number; endMs: number };
  window: Span;
  /** Notes per equal-width bucket over the span; null while unknown. */
  density: readonly number[] | null;
  labelled: readonly Span[];
  /** A saved window keeps the anchor its label was stored with. */
  locked: boolean;
  /** The new window start, committed once per gesture or key. */
  onMove: (t0Ms: number) => void;
  params?: ChartTimelineParams;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function percent(fraction: number): string {
  return `${String(Math.round(fraction * 10_000) / 100)}%`;
}

export function ChartTimeline(props: ChartTimelineProps) {
  const { span, window: current, density, labelled, locked, onMove, params = CHART_TIMELINE_PARAMS } = props;
  const { t } = useTranslation();
  const lockedId = useId();
  const trackRef = useRef<HTMLDivElement>(null);
  const cleanup = useRef<(() => void) | null>(null);
  const [dragT0, setDragT0] = useState<number | null>(null);
  useEffect(
    () => () => {
      cleanup.current?.();
    },
    [],
  );

  const total = Math.max(1, span.endMs - span.firstMs);
  const length = current.t1Ms - current.t0Ms;
  const min = span.firstMs;
  const max = Math.max(min, span.endMs - length);
  const t0 = dragT0 ?? current.t0Ms;
  const at = (ms: number): number => (ms - span.firstMs) / total;

  const commit = (next: number): void => {
    const target = Math.round(clamp(next, min, max));
    if (target !== current.t0Ms) {
      onMove(target);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>): void => {
    const next =
      e.key === "ArrowRight" || e.key === "ArrowUp"
        ? t0 + params.stepMs
        : e.key === "ArrowLeft" || e.key === "ArrowDown"
          ? t0 - params.stepMs
          : e.key === "PageUp"
            ? t0 + params.largeStepMs
            : e.key === "PageDown"
              ? t0 - params.largeStepMs
              : e.key === "Home"
                ? min
                : e.key === "End"
                  ? max
                  : null;
    if (next === null) {
      return;
    }
    e.preventDefault();
    if (!locked) {
      commit(next);
    }
  };

  const timeAt = (clientX: number): number => {
    const rect = trackRef.current?.getBoundingClientRect();
    if (rect === undefined || rect.width <= 0) {
      return t0;
    }
    return span.firstMs + clamp((clientX - rect.left) / rect.width, 0, 1) * total;
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>): void => {
    if (e.button !== 0 || locked) {
      return;
    }
    e.preventDefault();
    const grabbed = timeAt(e.clientX);
    const inside = grabbed >= current.t0Ms && grabbed <= current.t1Ms;
    // Outside the window a click centres it there; inside, the window follows the pointer from where it was grabbed.
    const grip = inside ? grabbed - current.t0Ms : length / 2;
    const place = (clientX: number): number => Math.round(clamp(timeAt(clientX) - grip, min, max));
    let last = place(e.clientX);
    setDragT0(last);
    // Window listeners, as the panel resizer: the pointer may leave the strip mid-drag and still steer it.
    const onMovePointer = (ev: PointerEvent): void => {
      last = place(ev.clientX);
      setDragT0(last);
    };
    const stop = (): void => {
      window.removeEventListener("pointermove", onMovePointer);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onCancel);
      cleanup.current = null;
      setDragT0(null);
    };
    const onUp = (): void => {
      stop();
      commit(last);
    };
    const onCancel = (): void => {
      stop();
    };
    cleanup.current?.();
    cleanup.current = stop;
    window.addEventListener("pointermove", onMovePointer);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onCancel);
  };

  const peak = density === null ? 0 : Math.max(0, ...density);
  const dragging = dragT0 !== null;

  return (
    <div role="group" aria-label={t("label.timeline.label")} className="flex flex-col gap-1">
      <div
        ref={trackRef}
        data-testid="timeline-track"
        onPointerDown={onPointerDown}
        className={cn(
          "bg-muted/40 relative h-10 touch-none overflow-hidden rounded-md border select-none",
          locked ? "cursor-not-allowed" : "cursor-pointer",
        )}
      >
        {density !== null && density.length > 0 && (
          <svg
            aria-hidden
            viewBox={`0 0 ${String(density.length)} 1`}
            preserveAspectRatio="none"
            className="absolute inset-0 size-full"
          >
            {density.map((count, i) => {
              const height = peak === 0 ? 0 : count / peak;
              return (
                <rect
                  // Buckets are positional and never reorder.
                  key={i}
                  data-testid="timeline-density-bar"
                  x={i}
                  y={1 - height}
                  width={1}
                  height={height}
                  className="fill-muted-foreground/35"
                />
              );
            })}
          </svg>
        )}
        {labelled.map((s) => (
          <div
            key={`${String(s.t0Ms)}-${String(s.t1Ms)}`}
            aria-hidden
            data-testid="timeline-labelled"
            style={{ left: percent(at(s.t0Ms)), width: percent((s.t1Ms - s.t0Ms) / total) }}
            className="bg-success/30 border-success/60 absolute inset-y-0 border-x"
          />
        ))}
        <div
          role="slider"
          tabIndex={0}
          aria-label={t("label.timeline.position")}
          aria-orientation="horizontal"
          aria-valuemin={min}
          aria-valuemax={max}
          aria-valuenow={t0}
          aria-valuetext={t("label.timeline.valueText", { from: formatMinSec(t0), to: formatMinSec(t0 + length) })}
          aria-disabled={locked || undefined}
          aria-describedby={locked ? lockedId : undefined}
          onKeyDown={onKeyDown}
          style={{ left: percent(at(t0)), width: percent(length / total) }}
          className={cn(
            "border-primary bg-primary/25 absolute inset-y-0 min-w-1.5 rounded-sm border-2 outline-none",
            "focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-1",
            !locked && "cursor-grab",
            dragging ? "cursor-grabbing" : "motion-safe:transition-[left] motion-safe:duration-150",
            locked && "border-muted-foreground bg-muted-foreground/20",
          )}
        />
      </div>
      <div className="text-muted-foreground flex justify-between font-mono text-[0.7rem] tabular-nums">
        <span>{formatMinSec(span.firstMs)}</span>
        <span>{t("label.timeline.labelledCount", { count: labelled.length })}</span>
        <span>{formatMinSec(span.endMs)}</span>
      </div>
      {locked && (
        <span id={lockedId} hidden>
          {t("label.timeline.locked")}
        </span>
      )}
    </div>
  );
}
