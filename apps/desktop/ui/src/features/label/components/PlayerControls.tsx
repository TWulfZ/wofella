import { Pause, Play, RotateCcw, RotateCw } from "lucide-react";
import { type ReactNode, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { Clock } from "@/features/playfield";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { formatClock } from "../format";

interface PlaybackTimeProps {
  clock: Clock | null;
  /** Shown before the first play, when there is no clock yet. */
  fallbackMs: number;
}

/**
 * The clock moves every frame; writing the text directly keeps React out of a 60 Hz render loop. Hidden from assistive
 * technology, which would otherwise be flooded; the window range beside it is the spoken position.
 */
function PlaybackTime({ clock, fallbackMs }: PlaybackTimeProps) {
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    let frame = 0;
    let shown = "";
    const tick = (): void => {
      const text = formatClock(clock?.nowMs() ?? fallbackMs);
      if (text !== shown && ref.current !== null) {
        ref.current.textContent = text;
        shown = text;
      }
      frame = requestAnimationFrame(tick);
    };
    tick();
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [clock, fallbackMs]);
  return <span ref={ref} aria-hidden data-testid="playback-time" className="font-mono text-sm tabular-nums" />;
}

interface PlayerControlsProps {
  playing: boolean;
  loading: boolean;
  onToggle: () => void;
  clock: Clock | null;
  windowStartMs: number;
  /** The window's bounds, `mm:ss.mmm–mm:ss.mmm`. */
  range: string;
  duration: string;
  rate: number;
  timeline: ReactNode;
  /** Back is the negative step; both wrap inside the section. */
  onSkip: (deltaMs: number) => void;
  skipMs: number;
}

const MS_PER_SECOND = 1000;

export function PlayerControls(props: PlayerControlsProps) {
  const { playing, loading, onToggle, clock, windowStartMs, range, duration, rate, timeline, onSkip, skipMs } = props;
  const { t } = useTranslation();
  const label = playing ? t("label.transport.pause") : t("label.transport.play");
  const seconds = String(skipMs / MS_PER_SECOND);
  const back = t("label.transport.back", { seconds });
  const forward = t("label.transport.forward", { seconds });
  return (
    <>
      {/* Transport and time sit on the right: the left edge belongs to the settings tab and its hover. */}
      <div className="flex items-center gap-3">
        {rate !== 1 && (
          <span
            title={t("label.transport.rate")}
            className="border-border bg-muted/70 rounded-full border px-2 font-mono text-xs font-semibold tabular-nums"
          >
            {t("label.transport.rateValue", { value: rate.toFixed(2) })}
          </span>
        )}
        <div data-testid="playback-readout" className="ml-auto flex min-w-0 flex-col items-end leading-tight">
          <span className="flex items-baseline gap-2">
            <span className="text-muted-foreground font-mono text-xs tabular-nums">{range}</span>
            <PlaybackTime clock={clock} fallbackMs={windowStartMs} />
          </span>
          <span className="text-muted-foreground text-xs">
            {loading && <>{t("label.transport.loadingAudio")} · </>}
            {duration}
          </span>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={back}
          title={back}
          onClick={() => {
            onSkip(-skipMs);
          }}
          className="shrink-0 rounded-full"
        >
          <RotateCcw aria-hidden />
        </Button>
        <Button
          type="button"
          size="icon-lg"
          aria-label={label}
          title={label}
          aria-keyshortcuts="Space"
          onClick={onToggle}
          className="size-10 shrink-0 rounded-full"
        >
          {playing ? <Pause aria-hidden className="fill-current" /> : <Play aria-hidden className="fill-current" />}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={forward}
          title={forward}
          onClick={() => {
            onSkip(skipMs);
          }}
          className="shrink-0 rounded-full"
        >
          <RotateCw aria-hidden />
        </Button>
      </div>
      {timeline}
    </>
  );
}

/** The paused section's call to action, over the middle of the stage. */
export function CentrePlayButton({ onPlay }: { onPlay: () => void }) {
  const { t } = useTranslation();
  const label = t("label.transport.playSection");
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-keyshortcuts="Space"
      onClick={onPlay}
      className={cn(
        "bg-primary/85 text-primary-foreground hover:bg-primary grid size-20 place-items-center rounded-full shadow-xl outline-none backdrop-blur-sm",
        "focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-4 focus-visible:ring-offset-2",
        "motion-safe:animate-in motion-safe:fade-in-0 motion-safe:zoom-in-90 motion-safe:duration-200",
      )}
    >
      <Play aria-hidden className="size-9 translate-x-0.5 fill-current" />
    </button>
  );
}
