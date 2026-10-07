import { Pause, Play } from "lucide-react";
import { type ReactNode, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { Clock } from "@/features/playfield";
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
}

export function PlayerControls(props: PlayerControlsProps) {
  const { playing, loading, onToggle, clock, windowStartMs, range, duration, rate, timeline } = props;
  const { t } = useTranslation();
  const label = playing ? t("label.transport.pause") : t("label.transport.play");
  return (
    <>
      <div className="flex items-center gap-3">
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
        <div className="flex min-w-0 flex-col leading-tight">
          <span className="flex items-baseline gap-2">
            <PlaybackTime clock={clock} fallbackMs={windowStartMs} />
            <span className="text-muted-foreground font-mono text-xs tabular-nums">{range}</span>
          </span>
          <span className="text-muted-foreground text-xs">
            {duration}
            {loading && <> · {t("label.transport.loadingAudio")}</>}
          </span>
        </div>
        {rate !== 1 && (
          <span
            title={t("label.transport.rate")}
            className="border-border bg-muted/70 ml-auto rounded-full border px-2 font-mono text-xs font-semibold tabular-nums"
          >
            {t("label.transport.rateValue", { value: rate.toFixed(2) })}
          </span>
        )}
      </div>
      {timeline}
    </>
  );
}
