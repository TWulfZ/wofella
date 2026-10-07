import {
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { formatClock, formatMinSec } from "../format";
import { skipWithin } from "../sectionPlayer";
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
  /** The playhead moves every frame; its spoken value is rewritten at most this often, so a screen reader is not flooded. */
  playheadAriaRefreshMs: number;
}

export const CHART_TIMELINE_PARAMS: ChartTimelineParams = {
  stepMs: 1000,
  largeStepMs: 5000,
  minWindowMs: 1000,
  maxWindowMs: 60_000,
  edgeOutsidePx: 8,
  edgeInsidePx: 6,
  minMoveGrabPx: 16,
  playheadAriaRefreshMs: 250,
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
  /** Playback inside the window; absent, no playhead is drawn. */
  playhead?: Playhead | undefined;
  params?: ChartTimelineParams;
}

export interface Playhead {
  /** Read every frame, so it must be cheap. */
  positionMs: () => number;
  /** The section playback loops over: the seek bar's range. */
  loop: { startMs: number; endMs: number };
  /** Arrow-key step, wrapping inside the loop. */
  stepMs: number;
  onSeek: (chartMs: number) => void;
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

interface SeekBarProps {
  playhead: Playhead;
  window: Span;
  /** The chart track's playhead line, positioned on the same frame as the bar's. */
  lineRef: RefObject<HTMLDivElement | null>;
  chartFraction: (ms: number) => number;
  params: ChartTimelineParams;
}

/** A video player's progress bar, zoomed to the section: on the whole-chart track a short window is a few px wide. */
function SeekBar({ playhead, window: current, lineRef, chartFraction, params }: SeekBarProps) {
  const { t } = useTranslation();
  const { loop, positionMs, onSeek, stepMs } = playhead;
  const barRef = useRef<HTMLDivElement>(null);
  const knobRef = useRef<HTMLDivElement>(null);
  const progressRef = useRef<HTMLDivElement>(null);
  const cleanup = useRef<(() => void) | null>(null);
  // The pointer's time while dragging, shown in place of playback; committed on release.
  const dragMs = useRef<number | null>(null);
  // Set by a seek so the spoken value updates on the next frame instead of after the refresh interval.
  const spokenStale = useRef(true);
  const len = Math.max(1, loop.endMs - loop.startMs);
  const loopStart = loop.startMs;
  const fraction = useCallback((ms: number): number => clamp((ms - loopStart) / len, 0, 1), [loopStart, len]);

  useEffect(
    () => () => {
      cleanup.current?.();
    },
    [],
  );

  // Writes the DOM directly: the position changes every frame, which must not re-render React.
  const paint = useRef<(ms: number) => void>(() => undefined);
  useEffect(() => {
    let frame = 0;
    let spokenAt = Number.NEGATIVE_INFINITY;
    let spoken = "";
    paint.current = (ms: number): void => {
      const at = percent(fraction(ms));
      if (knobRef.current !== null) {
        knobRef.current.style.left = at;
        const text = formatClock(ms);
        const now = performance.now();
        if (text !== spoken && (spokenStale.current || now - spokenAt >= params.playheadAriaRefreshMs)) {
          knobRef.current.setAttribute("aria-valuenow", String(Math.round(ms)));
          knobRef.current.setAttribute("aria-valuetext", text);
          spoken = text;
          spokenAt = now;
          spokenStale.current = false;
        }
      }
      if (progressRef.current !== null) {
        progressRef.current.style.width = at;
      }
      if (lineRef.current !== null) {
        lineRef.current.style.left = percent(chartFraction(ms));
      }
    };
    const tick = (): void => {
      paint.current(dragMs.current ?? positionMs());
      frame = requestAnimationFrame(tick);
    };
    tick();
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [positionMs, fraction, lineRef, params.playheadAriaRefreshMs, chartFraction]);

  const seek = (ms: number): void => {
    spokenStale.current = true;
    onSeek(Math.round(ms));
  };

  const timeAt = (clientX: number): number => {
    const rect = barRef.current?.getBoundingClientRect();
    if (rect === undefined || rect.width <= 0) {
      return positionMs();
    }
    return loop.startMs + clamp((clientX - rect.left) / rect.width, 0, 1) * len;
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>): void => {
    if (e.button !== 0) {
      return;
    }
    e.preventDefault();
    const first = timeAt(e.clientX);
    dragMs.current = first;
    seek(first);
    paint.current(first);
    let last = first;
    cleanup.current?.();
    cleanup.current = trackPointer(
      (clientX) => {
        last = timeAt(clientX);
        dragMs.current = last;
        paint.current(last);
      },
      () => {
        if (last !== first) {
          seek(last);
        }
      },
      () => {
        cleanup.current = null;
        dragMs.current = null;
      },
    );
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>): void => {
    const step = keyStep(e.key, { ...params, stepMs, largeStepMs: stepMs });
    if (step !== null) {
      e.preventDefault();
      seek(skipWithin(positionMs(), step, loop));
    } else if (e.key === "Home") {
      e.preventDefault();
      seek(loop.startMs);
    } else if (e.key === "End") {
      e.preventDefault();
      seek(loop.endMs);
    }
  };

  const initial = positionMs();
  return (
    <div
      ref={barRef}
      data-testid="seek-bar"
      onPointerDown={onPointerDown}
      className="group/seek relative h-4 cursor-pointer touch-none select-none"
    >
      <div aria-hidden className="bg-muted/70 absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 rounded-full" />
      <div
        aria-hidden
        data-testid="seek-window"
        style={{ left: percent(fraction(current.t0Ms)), width: percent(fraction(current.t1Ms) - fraction(current.t0Ms)) }}
        className="bg-primary/30 absolute top-1/2 h-1 -translate-y-1/2"
      />
      <div
        ref={progressRef}
        aria-hidden
        className="bg-primary absolute top-1/2 left-0 h-1 -translate-y-1/2 rounded-full group-hover/seek:h-1.5"
      />
      <div
        ref={knobRef}
        role="slider"
        tabIndex={0}
        aria-label={t("label.timeline.playhead")}
        aria-orientation="horizontal"
        aria-valuemin={loop.startMs}
        aria-valuemax={loop.endMs}
        aria-valuenow={Math.round(initial)}
        aria-valuetext={formatClock(initial)}
        onKeyDown={onKeyDown}
        style={{ left: percent(fraction(initial)) }}
        className={cn(
          "bg-primary border-background absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 shadow outline-none",
          "focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-1",
        )}
      />
    </div>
  );
}

export function ChartTimeline(props: ChartTimelineProps) {
  const { span, window: current, density, labelled, locked, onMove, onResize, playhead } = props;
  const params = props.params ?? CHART_TIMELINE_PARAMS;
  const { t } = useTranslation();
  const lockedId = useId();
  const trackRef = useRef<HTMLDivElement>(null);
  const lineRef = useRef<HTMLDivElement>(null);
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
  const chartFraction = useCallback((ms: number): number => clamp((ms - span.firstMs) / total, 0, 1), [span.firstMs, total]);

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
      {playhead !== undefined && (
        <SeekBar playhead={playhead} window={shown} lineRef={lineRef} chartFraction={chartFraction} params={params} />
      )}
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
        {playhead !== undefined && (
          <div
            ref={lineRef}
            aria-hidden="true"
            data-testid="timeline-playhead-line"
            style={{ left: percent(chartFraction(playhead.positionMs())) }}
            className="bg-foreground pointer-events-none absolute inset-y-0 z-10 w-px"
          />
        )}
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
