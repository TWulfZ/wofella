import {
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { clampOsuSpeed, DEFAULT_STAGE_PARAMS } from "@/features/playfield";
import { LABEL_PREFS, type ScrollKind, type ScrollPrefs } from "../prefs";
import { SkinPicker, type SkinPickerProps } from "./SkinPicker";

const SCROLL_STEP = 0.05;
const ZOOM_STEP = 0.05;
const SCROLL_KINDS = [
  { kind: "osu", labelKey: "label.transport.modeOsu" },
  { kind: "pxPerMs", labelKey: "label.transport.modePxPerMs" },
] as const satisfies readonly { kind: ScrollKind; labelKey: string }[];

const FIELD = "border-input bg-background h-8 rounded-md border px-2 text-sm";

interface SettingProps {
  id: string;
  label: string;
  value?: ReactNode;
  children: ReactNode;
}

function Setting({ id, label, value, children }: SettingProps) {
  return (
    <div className="flex flex-col gap-1.5 text-sm">
      <div className="flex items-baseline justify-between gap-2">
        <label htmlFor={id} className="text-muted-foreground">
          {label}
        </label>
        {value !== undefined && (
          <output htmlFor={id} className="font-mono text-xs tabular-nums">
            {value}
          </output>
        )}
      </div>
      {children}
    </div>
  );
}

/**
 * Keeps a range's value local while a pointer drags it and commits once on release; a keyboard step commits at once.
 * For settings whose commit is expensive: a playback rate change rebuilds the playing loop.
 */
function useCommitOnRelease(value: number, onCommit: (value: number) => void) {
  const [draft, setDraft] = useState<number | null>(null);
  const dragged = useRef<number | null>(null);
  const stopTracking = useRef<(() => void) | null>(null);
  const commit = useRef(onCommit);
  useLayoutEffect(() => {
    commit.current = onCommit;
  });
  useEffect(
    () => () => {
      stopTracking.current?.();
    },
    [],
  );

  const onPointerDown = (e: ReactPointerEvent<HTMLInputElement>): void => {
    if (e.button !== 0 || stopTracking.current !== null) {
      return;
    }
    // On window: the release may land off the thumb, or off the flyout.
    const stop = (): void => {
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      stopTracking.current = null;
    };
    const end = (): void => {
      stop();
      const last = dragged.current;
      dragged.current = null;
      setDraft(null);
      if (last !== null) {
        commit.current(last);
      }
    };
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    stopTracking.current = stop;
  };

  const onChange = (next: number): void => {
    if (stopTracking.current === null) {
      onCommit(next);
      return;
    }
    dragged.current = next;
    setDraft(next);
  };

  return { shown: draft ?? value, onPointerDown, onChange };
}

interface PlaybackSettingsProps {
  offsetMs: number;
  onOffset: (offsetMs: number) => void;
  scroll: ScrollPrefs;
  onScroll: (patch: Partial<ScrollPrefs>) => void;
  zoom: number;
  onZoom: (zoom: number) => void;
  rate: number;
  onRate: (rate: number) => void;
  skin: SkinPickerProps;
  seed: string;
}

export function PlaybackSettings(props: PlaybackSettingsProps) {
  const { offsetMs, onOffset, scroll, onScroll, zoom, onZoom, rate, onRate, skin, seed } = props;
  const { t } = useTranslation();
  const offsetId = useId();
  const rateId = useId();
  const modeId = useId();
  const speedId = useId();
  const zoomId = useId();
  const rateDrag = useCommitOnRelease(rate, onRate);
  return (
    <>
      <Setting
        id={offsetId}
        label={t("label.transport.offset")}
        value={t("label.transport.offsetValue", { value: offsetMs > 0 ? `+${String(offsetMs)}` : String(offsetMs) })}
      >
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
          className="w-full accent-current"
        />
      </Setting>

      <Setting
        id={rateId}
        label={t("label.transport.rate")}
        value={t("label.transport.rateValue", { value: rateDrag.shown.toFixed(2) })}
      >
        <input
          id={rateId}
          type="range"
          min={LABEL_PREFS.minPlaybackRate}
          max={LABEL_PREFS.maxPlaybackRate}
          step={LABEL_PREFS.playbackRateStep}
          value={rateDrag.shown}
          onPointerDown={rateDrag.onPointerDown}
          onChange={(e) => {
            rateDrag.onChange(Number(e.target.value));
          }}
          className="w-full accent-current"
        />
      </Setting>

      <Setting id={modeId} label={t("label.transport.scrollMode")}>
        <select
          id={modeId}
          value={scroll.kind}
          onChange={(e) => {
            const kind = SCROLL_KINDS.find((k) => k.kind === e.target.value)?.kind;
            if (kind !== undefined) {
              onScroll({ kind });
            }
          }}
          className={FIELD}
        >
          {SCROLL_KINDS.map(({ kind, labelKey }) => (
            <option key={kind} value={kind}>
              {t(labelKey)}
            </option>
          ))}
        </select>
      </Setting>

      {scroll.kind === "osu" ? (
        <Setting id={speedId} label={t("label.transport.osuSpeed")}>
          <input
            id={speedId}
            type="number"
            min={DEFAULT_STAGE_PARAMS.minOsuSpeed}
            max={DEFAULT_STAGE_PARAMS.maxOsuSpeed}
            step={DEFAULT_STAGE_PARAMS.osuSpeedStep}
            value={scroll.osuSpeed}
            disabled={scroll.fit}
            aria-keyshortcuts="F3 F4"
            title={t("label.transport.osuSpeedHint")}
            onChange={(e) => {
              if (e.target.value !== "") {
                onScroll({ osuSpeed: clampOsuSpeed(Number(e.target.value)) });
              }
            }}
            className={`${FIELD} w-20 font-mono tabular-nums disabled:opacity-40`}
          />
        </Setting>
      ) : (
        <Setting
          id={speedId}
          label={t("label.transport.scroll")}
          value={t("label.transport.scrollValue", { value: scroll.pxPerMs.toFixed(2) })}
        >
          <input
            id={speedId}
            type="range"
            min={LABEL_PREFS.minPxPerMs}
            max={LABEL_PREFS.maxPxPerMs}
            step={SCROLL_STEP}
            value={scroll.pxPerMs}
            disabled={scroll.fit}
            onChange={(e) => {
              onScroll({ pxPerMs: Number(e.target.value) });
            }}
            className="w-full accent-current disabled:opacity-40"
          />
        </Setting>
      )}

      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={scroll.fit}
          onChange={(e) => {
            onScroll({ fit: e.target.checked });
          }}
          className="focus-visible:ring-ring size-4 outline-none focus-visible:ring-2"
        />
        {t("label.transport.fit")}
      </label>

      <Setting id={zoomId} label={t("label.transport.zoom")} value={t("label.transport.zoomValue", { value: zoom.toFixed(2) })}>
        <input
          id={zoomId}
          type="range"
          min={DEFAULT_STAGE_PARAMS.minZoom}
          max={DEFAULT_STAGE_PARAMS.maxZoom}
          step={ZOOM_STEP}
          value={zoom}
          onChange={(e) => {
            onZoom(Number(e.target.value));
          }}
          className="w-full accent-current"
        />
      </Setting>

      <SkinPicker {...skin} />

      <p className="text-muted-foreground mt-auto font-mono text-xs">{t("label.player.seed", { seed })}</p>
    </>
  );
}
