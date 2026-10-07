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
import { clampOsuSpeed, DEFAULT_STAGE_PARAMS, type PlayfieldEffects } from "@/features/playfield";
import { LABEL_PREFS, type ScrollKind, type ScrollPrefs } from "../prefs";
import { SkinPicker, type SkinPickerProps } from "./SkinPicker";

const SCROLL_STEP = 0.05;
const ZOOM_STEP = 0.05;
const SCROLL_KINDS = [
  { kind: "osu", labelKey: "label.transport.modeOsu" },
  { kind: "pxPerMs", labelKey: "label.transport.modePxPerMs" },
] as const satisfies readonly { kind: ScrollKind; labelKey: string }[];

const FIELD = "border-input bg-background/80 h-8 rounded-md border px-2 text-sm";

// Labels sit on the flyout's translucent surface, where muted text would fall under 4.5:1 over a bright stage.
const LABEL_TEXT = "text-foreground/90";

type EffectId = keyof PlayfieldEffects;

/**
 * Display order; "inverted" marks the toggle that reads as the effect's negation. "textFallback" marks the effects the
 * playfield draws as text when the skin lacks their art, so they stay on offer and only say so. "autoplay" marks the
 * effects driven by the simulated autoplay rather than by the chart alone.
 */
const EFFECT_TOGGLES = [
  { id: "percy", labelKey: "label.effects.percyOff", inverted: true, textFallback: false, autoplay: false },
  { id: "judgements", labelKey: "label.effects.judgements", inverted: false, textFallback: true, autoplay: true },
  { id: "combo", labelKey: "label.effects.combo", inverted: false, textFallback: true, autoplay: true },
  { id: "keyPress", labelKey: "label.effects.keyPress", inverted: false, textFallback: false, autoplay: true },
  { id: "lighting", labelKey: "label.effects.lighting", inverted: false, textFallback: false, autoplay: true },
] as const satisfies readonly {
  id: EffectId;
  labelKey: string;
  inverted: boolean;
  textFallback: boolean;
  autoplay: boolean;
}[];

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
        <label htmlFor={id} className={LABEL_TEXT}>
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
  effects: PlayfieldEffects;
  onEffects: (patch: Partial<PlayfieldEffects>) => void;
  /** Whether the loaded skin has what each effect draws with. */
  effectSupport: Record<EffectId, boolean>;
  /** The chosen skin is still on its way, so `effectSupport` describes the procedural stage, not the skin. */
  skinLoading?: boolean;
  seed: string;
}

interface EffectToggleProps {
  label: string;
  checked: boolean;
  disabled: boolean;
  /** Why the toggle is disabled, or how it draws without the skin's art; null when there is nothing to say. */
  note: string | null;
  onChange: (checked: boolean) => void;
}

function EffectToggle({ label, checked, disabled, note, onChange }: EffectToggleProps) {
  const id = useId();
  const reasonId = useId();
  return (
    <div className="flex flex-col gap-0.5">
      <label htmlFor={id} title={note ?? undefined} className="flex items-center gap-2 text-sm has-disabled:opacity-60">
        <input
          id={id}
          type="checkbox"
          // A disabled effect draws nothing, so it never reads as on, though the choice is kept for skins that can.
          checked={checked && !disabled}
          disabled={disabled}
          aria-describedby={note === null ? undefined : reasonId}
          onChange={(e) => {
            onChange(e.target.checked);
          }}
          className="focus-visible:ring-ring size-4 outline-none focus-visible:ring-2"
        />
        {label}
      </label>
      {note !== null && (
        <p id={reasonId} className={`${LABEL_TEXT} pl-6 text-xs`}>
          {note}
        </p>
      )}
    </div>
  );
}

function SkinEffects({
  effects,
  onEffects,
  effectSupport,
  skinLoading,
}: Pick<PlaybackSettingsProps, "effects" | "onEffects" | "effectSupport"> & { skinLoading: boolean }) {
  const { t } = useTranslation();
  const drawable = (toggle: (typeof EFFECT_TOGGLES)[number]): boolean => effectSupport[toggle.id] || toggle.textFallback;
  // Only an effect that is actually drawn follows the autoplay; a chosen one the skin cannot draw shows nothing.
  const autoplay = EFFECT_TOGGLES.some((toggle) => toggle.autoplay && effects[toggle.id] && drawable(toggle));
  const note = ({ id, textFallback }: (typeof EFFECT_TOGGLES)[number]): string | null => {
    if (effectSupport[id]) {
      return null;
    }
    if (textFallback) {
      return t(`label.effects.textFallback.${id}`);
    }
    return skinLoading ? null : t(`label.effects.unsupported.${id}`);
  };
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className={`${LABEL_TEXT} mb-1.5 text-sm`}>{t("label.effects.title")}</legend>
      {skinLoading && (
        <p role="status" className={`${LABEL_TEXT} text-xs`}>
          {t("label.effects.skinLoading")}
        </p>
      )}
      {EFFECT_TOGGLES.map((toggle) => (
        <EffectToggle
          key={toggle.id}
          label={t(toggle.labelKey)}
          checked={toggle.inverted ? !effects[toggle.id] : effects[toggle.id]}
          disabled={!drawable(toggle)}
          note={note(toggle)}
          onChange={(checked) => {
            onEffects({ [toggle.id]: toggle.inverted ? !checked : checked });
          }}
        />
      ))}
      {autoplay && <p className={`${LABEL_TEXT} text-xs`}>{t("label.effects.autoplay")}</p>}
    </fieldset>
  );
}

export function PlaybackSettings(props: PlaybackSettingsProps) {
  const {
    offsetMs,
    onOffset,
    scroll,
    onScroll,
    zoom,
    onZoom,
    rate,
    onRate,
    skin,
    effects,
    onEffects,
    effectSupport,
    skinLoading = false,
    seed,
  } = props;
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
            aria-keyshortcuts="F3 F4"
            title={t("label.transport.osuSpeedHint")}
            onChange={(e) => {
              if (e.target.value !== "") {
                onScroll({ osuSpeed: clampOsuSpeed(Number(e.target.value)) });
              }
            }}
            className={`${FIELD} w-20 font-mono tabular-nums`}
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
            onChange={(e) => {
              onScroll({ pxPerMs: Number(e.target.value) });
            }}
            className="w-full accent-current"
          />
        </Setting>
      )}

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
      <SkinEffects effects={effects} onEffects={onEffects} effectSupport={effectSupport} skinLoading={skinLoading} />

      <p className="text-foreground/80 mt-auto font-mono text-xs">{t("label.player.seed", { seed })}</p>
    </>
  );
}
