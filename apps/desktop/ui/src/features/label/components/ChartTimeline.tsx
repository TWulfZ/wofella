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
  /** Window length bounds; the same as the backend's LabelingParams, which clamps again on resize. */
  minWindowMs: number;
  maxWindowMs: number;
  /** Grab width of each resize handle, drawn just outside the window edge. */
  edgeOutsidePx: number;
  /** How far inside the window an edge still grabs, while the body is wide enough to spare it. */
  edgeInsidePx: number;
  /** The move grab never shrinks below this, so a few-px window on a long chart can still be dragged. */
  minMoveGrabPx: number;
}

export const CHART_TIMELINE_PARAMS: ChartTimelineParams = {
  stepMs: 1000,
  largeStepMs: 5000,
  minWindowMs: 1000,
  maxWindowMs: 60_000,
  edgeOutsidePx: 8,
  edgeInsidePx: 6,
  minMoveGrabPx: 16,
};

type Grab = "move" | Edge | "outside";

/**
 * What a press at `x` grabs, in px along the track. Decided by position rather than by which element's box is on top,
 * because on a narrow window the handles' boxes would otherwise cover the whole body.
 */
function grabAt(x: number, startPx: number, endPx: number, params: ChartTimelineParams): Grab {
  const centre = (startPx + endPx) / 2;
  const half = Math.max(params.minMoveGrabPx / 2, (endPx - startPx) / 2 - params.edgeInsidePx);
  if (Math.abs(x - centre) <= half) {
    return "move";
  }
  if (x < centre && x >= Math.min(startPx, centre - half) - params.edgeOutsidePx) {
    return "start";
  }
  if (x > centre && x <= Math.max(endPx, centre + half) + params.edgeOutsidePx) {
    return "end";
  }
  return "outside";
}

type Edge = "start" | "end";

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
  /** The window with one edge moved, committed once per gesture or key. */
  onResize: (span: Span) => void;
  params?: ChartTimelineParams;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function percent(fraction: number): string {
  return `${String(Math.round(fraction * 10_000) / 100)}%`;
}

/** The step a key asks for, or null for a key the sliders do not handle. Home/End are handled by each caller. */
function keyStep(key: string, params: ChartTimelineParams): number | null {
  switch (key) {
    case "ArrowRight":
    case "ArrowUp":
      return params.stepMs;
    case "ArrowLeft":
    case "ArrowDown":
      return -params.stepMs;
    case "PageUp":
      return params.largeStepMs;
    case "PageDown":
      return -params.largeStepMs;
    default:
      return null;
  }
}

/**
 * Follows the pointer on window listeners, as the panel resizer does: the pointer may leave the strip mid-drag and
 * still steer it. Returns the teardown.
 */
function trackPointer(onMove: (clientX: number) => void, onUp: () => void, onEnd: () => void): () => void {
  const move = (ev: PointerEvent): void => {
    onMove(ev.clientX);
  };
  const stop = (): void => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", stop);
    onEnd();
  };
  const up = (): void => {
    stop();
    onUp();
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("pointercancel", stop);
  return stop;
}

export function ChartTimeline(props: ChartTimelineProps) {
  const { span, window: current, density, labelled, locked, onMove, onResize, params = CHART_TIMELINE_PARAMS } = props;
  const { t } = useTranslation();
  const lockedId = useId();
  const trackRef = useRef<HTMLDivElement>(null);
  const cleanup = useRef<(() => void) | null>(null);
  // The span on screen while a gesture runs; committed on release.
  const [drag, setDrag] = useState<Span | null>(null);
  useEffect(
    () => () => {
      cleanup.current?.();
    },
    [],
  );

  const total = Math.max(1, span.endMs - span.firstMs);
  const shown = drag ?? current;
  const length = shown.t1Ms - shown.t0Ms;
  const min = span.firstMs;
  const max = Math.max(min, span.endMs - (current.t1Ms - current.t0Ms));
  const at = (ms: number): number => (ms - span.firstMs) / total;

  // Each edge moves between the length bounds measured from the other edge, never outside the chart.
  const startMin = Math.max(span.firstMs, current.t1Ms - params.maxWindowMs);
  const startMax = Math.max(startMin, current.t1Ms - params.minWindowMs);
  const endMin = Math.min(span.endMs, current.t0Ms + params.minWindowMs);
  const endMax = Math.max(endMin, Math.min(span.endMs, current.t0Ms + params.maxWindowMs));
  const resized = (edge: Edge, ms: number): Span =>
    edge === "start"
      ? { t0Ms: Math.round(clamp(ms, startMin, startMax)), t1Ms: current.t1Ms }
      : { t0Ms: current.t0Ms, t1Ms: Math.round(clamp(ms, endMin, endMax)) };

  const commitMove = (next: number): void => {
    const target = Math.round(clamp(next, min, max));
    if (target !== current.t0Ms) {
      onMove(target);
    }
  };
  const commitResize = (next: Span): void => {
    if (next.t0Ms !== current.t0Ms || next.t1Ms !== current.t1Ms) {
      onResize(next);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>): void => {
    const step = keyStep(e.key, params);
    const next = step !== null ? shown.t0Ms + step : e.key === "Home" ? min : e.key === "End" ? max : null;
    if (next === null) {
      return;
    }
    e.preventDefault();
    if (!locked) {
      commitMove(next);
    }
  };

  const onEdgeKeyDown = (edge: Edge) => (e: KeyboardEvent<HTMLDivElement>): void => {
    const edgeMs = edge === "start" ? current.t0Ms : current.t1Ms;
    const step = keyStep(e.key, params);
    const next =
      step !== null
        ? edgeMs + step
        : e.key === "Home"
          ? edge === "start"
            ? startMin
            : endMin
          : e.key === "End"
            ? edge === "start"
              ? startMax
              : endMax
            : null;
    if (next === null) {
      return;
    }
    e.preventDefault();
    if (!locked) {
      commitResize(resized(edge, next));
    }
  };

  const timeAt = (clientX: number): number => {
    const rect = trackRef.current?.getBoundingClientRect();
    if (rect === undefined || rect.width <= 0) {
      return shown.t0Ms;
    }
    return span.firstMs + clamp((clientX - rect.left) / rect.width, 0, 1) * total;
  };

  const startGesture = (first: Span, follow: (clientX: number) => Span, commit: (last: Span) => void): void => {
    let last = first;
    setDrag(last);
    cleanup.current?.();
    cleanup.current = trackPointer(
      (clientX) => {
        last = follow(clientX);
        setDrag(last);
      },
      () => {
        commit(last);
      },
      () => {
        cleanup.current = null;
        setDrag(null);
      },
    );
  };

  const grabOf = (clientX: number): Grab => {
    const rect = trackRef.current?.getBoundingClientRect();
    if (rect === undefined || rect.width <= 0) {
      return "move";
    }
    const px = (ms: number): number => rect.left + at(ms) * rect.width;
    return grabAt(clientX, px(current.t0Ms), px(current.t1Ms), params);
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>): void => {
    if (e.button !== 0 || locked) {
      return;
    }
    e.preventDefault();
    const grab = grabOf(e.clientX);
    const grabbed = timeAt(e.clientX);
    if (grab === "start" || grab === "end") {
      // The edge keeps its offset from the pointer, so a press beside it does not make it jump.
      const slip = grabbed - (grab === "start" ? current.t0Ms : current.t1Ms);
      const follow = (clientX: number): Span => resized(grab, timeAt(clientX) - slip);
      startGesture(follow(e.clientX), follow, commitResize);
      return;
    }
    const windowLength = current.t1Ms - current.t0Ms;
    // Outside the window a click centres it there; on it, the window follows the pointer from where it was grabbed.
    const grip = grab === "move" ? grabbed - current.t0Ms : windowLength / 2;
    const place = (clientX: number): Span => {
      const t0Ms = Math.round(clamp(timeAt(clientX) - grip, min, max));
      return { t0Ms, t1Ms: t0Ms + windowLength };
    };
    startGesture(place(e.clientX), place, (last) => {
      commitMove(last.t0Ms);
    });
  };

  const peak = density === null ? 0 : Math.max(0, ...density);
  const dragging = drag !== null;
  const lengthText = ((current.t1Ms - current.t0Ms) / 1000).toFixed(1);

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
          aria-valuenow={shown.t0Ms}
          aria-valuetext={t("label.timeline.valueText", { from: formatMinSec(shown.t0Ms), to: formatMinSec(shown.t1Ms) })}
          aria-disabled={locked || undefined}
          aria-describedby={locked ? lockedId : undefined}
          onKeyDown={onKeyDown}
          style={{ left: percent(at(shown.t0Ms)), width: percent(length / total) }}
          className={cn(
            "border-primary bg-primary/25 absolute inset-y-0 min-w-1.5 rounded-sm border-2 outline-none",
            "focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-1",
            !locked && "cursor-grab",
            dragging ? "cursor-grabbing" : "motion-safe:transition-[left,width] motion-safe:duration-150",
            locked && "border-muted-foreground bg-muted-foreground/20",
          )}
        />
        {(["start", "end"] as const).map((edge) => {
          const edgeMs = edge === "start" ? shown.t0Ms : shown.t1Ms;
          return (
            <div
              key={edge}
              role="slider"
              tabIndex={0}
              aria-label={t(edge === "start" ? "label.timeline.start" : "label.timeline.end")}
              aria-orientation="horizontal"
              aria-valuemin={edge === "start" ? startMin : endMin}
              aria-valuemax={edge === "start" ? startMax : endMax}
              aria-valuenow={edgeMs}
              aria-valuetext={t("label.timeline.edgeValueText", { time: formatMinSec(edgeMs), seconds: lengthText })}
              aria-disabled={locked || undefined}
              aria-describedby={locked ? lockedId : undefined}
              data-edge={edge}
              onKeyDown={onEdgeKeyDown(edge)}
              style={{ left: percent(at(edgeMs)) }}
              className={cn(
                // Outside the window, so a narrow window's body is never covered; presses are hit-tested by the track.
                "group/edge absolute inset-y-0 z-10 flex w-2 outline-none",
                edge === "start" ? "-translate-x-full justify-end" : "justify-start",
                locked ? "cursor-not-allowed" : "cursor-ew-resize",
                !dragging && "motion-safe:transition-[left] motion-safe:duration-150",
              )}
            >
              <span
                aria-hidden
                className={cn(
                  "my-1.5 w-1 rounded-full",
                  locked ? "bg-muted-foreground/60" : "bg-primary group-hover/edge:w-1.5",
                  "group-focus-visible/edge:ring-ring group-focus-visible/edge:ring-offset-background group-focus-visible/edge:w-1.5 group-focus-visible/edge:ring-2 group-focus-visible/edge:ring-offset-1",
                )}
              />
            </div>
          );
        })}
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
