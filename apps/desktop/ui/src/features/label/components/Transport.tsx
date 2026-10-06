import { Pause, Play } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/shared/ui/button";
import { LABEL_PREFS, type ScrollPref } from "../prefs";

const SCROLL_STEP = 0.05;

interface TransportProps {
  playing: boolean;
  loading: boolean;
  onToggle: () => void;
  range: string;
  duration: string;
  offsetMs: number;
  onOffset: (offsetMs: number) => void;
  scroll: ScrollPref;
  fixedSpeed: number;
  onScroll: (scroll: ScrollPref) => void;
  /** After a control here was used with the pointer or changed, so the screen can take focus back. */
  onSettle: () => void;
}

export function Transport(props: TransportProps) {
  const { playing, loading, onToggle, range, duration, offsetMs, onOffset, scroll, fixedSpeed, onScroll, onSettle } = props;
  const { t } = useTranslation();
  const offsetId = useId();
  const scrollId = useId();
  const fit = scroll === "fit";
  return (
    <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
      <Button size="lg" aria-keyshortcuts="Space" onClick={onToggle} className="min-w-28">
        {playing ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
        {playing ? t("label.transport.pause") : t("label.transport.play")}
      </Button>
      <div className="flex flex-col leading-tight">
        <span className="font-mono text-sm tabular-nums">{range}</span>
        <span className="text-muted-foreground text-xs">
          {duration}
          {loading && <> · {t("label.transport.loadingAudio")}</>}
        </span>
      </div>
      <div className="flex items-center gap-2 text-sm">
        <label htmlFor={offsetId} className="text-muted-foreground">
          {t("label.transport.offset")}
        </label>
        <input
          id={offsetId}
          type="range"
          min={LABEL_PREFS.minOffsetMs}
          max={LABEL_PREFS.maxOffsetMs}
          step={1}
          value={offsetMs}
          onChange={(e) => {
            onOffset(Number(e.target.value));
          }}
          onPointerUp={onSettle}
          className="w-28 accent-current"
        />
        <output htmlFor={offsetId} className="w-14 font-mono text-xs tabular-nums">
          {t("label.transport.offsetValue", { value: offsetMs > 0 ? `+${offsetMs}` : String(offsetMs) })}
        </output>
      </div>
      <div className="flex items-center gap-2 text-sm">
        <label className="flex items-center gap-1.5">
          <input
            type="checkbox"
            checked={fit}
            onChange={(e) => {
              onScroll(e.target.checked ? "fit" : fixedSpeed);
              onSettle();
            }}
          />
          {t("label.transport.fit")}
        </label>
        <label htmlFor={scrollId} className="text-muted-foreground">
          {t("label.transport.scroll")}
        </label>
        <input
          id={scrollId}
          type="range"
          min={LABEL_PREFS.minPxPerMs}
          max={LABEL_PREFS.maxPxPerMs}
          step={SCROLL_STEP}
          value={fixedSpeed}
          disabled={fit}
          onChange={(e) => {
            onScroll(Number(e.target.value));
          }}
          onPointerUp={onSettle}
          className="w-24 accent-current disabled:opacity-40"
        />
        <output htmlFor={scrollId} className="w-16 font-mono text-xs tabular-nums">
          {t("label.transport.scrollValue", { value: fixedSpeed.toFixed(2) })}
        </output>
      </div>
    </div>
  );
}
