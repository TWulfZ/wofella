import {
  ChevronsLeft,
  ChevronsRight,
  FoldHorizontal,
  type LucideIcon,
  Pause,
  Play,
  UnfoldHorizontal,
} from "lucide-react";
import { type KeyboardEvent, useId } from "react";
import { useTranslation } from "react-i18next";
import { clampOsuSpeed, DEFAULT_STAGE_PARAMS } from "@/features/playfield";
import { Button } from "@/shared/ui/button";
import { LABEL_PREFS, type ScrollKind, type ScrollPrefs } from "../prefs";
import type { WindowOp } from "../types";

const SCROLL_STEP = 0.05;
const ZOOM_STEP = 0.05;
const SCROLL_KINDS = [
  { kind: "osu", labelKey: "label.transport.modeOsu" },
  { kind: "pxPerMs", labelKey: "label.transport.modePxPerMs" },
] as const satisfies readonly { kind: ScrollKind; labelKey: string }[];
const RESHAPES: readonly { op: WindowOp; icon: LucideIcon; labelKey: string }[] = [
  { op: "widen", icon: UnfoldHorizontal, labelKey: "label.reshape.widen" },
  { op: "narrow", icon: FoldHorizontal, labelKey: "label.reshape.narrow" },
  { op: "prev", icon: ChevronsLeft, labelKey: "label.reshape.earlier" },
  { op: "next", icon: ChevronsRight, labelKey: "label.reshape.later" },
];

interface TransportProps {
  playing: boolean;
  loading: boolean;
  onToggle: () => void;
  range: string;
  duration: string;
  offsetMs: number;
  onOffset: (offsetMs: number) => void;
  scroll: ScrollPrefs;
  onScroll: (patch: Partial<ScrollPrefs>) => void;
  zoom: number;
  onZoom: (zoom: number) => void;
  /** After a control here was used with the pointer or changed, so the screen can take focus back. */
  onSettle: () => void;
  onReshape: (op: WindowOp) => void;
  reshapeDisabled: boolean;
}

export function Transport(props: TransportProps) {
  const { playing, loading, onToggle, range, duration, offsetMs, onOffset, scroll, onScroll, zoom, onZoom, onSettle } =
    props;
  const { onReshape, reshapeDisabled } = props;
  const { t } = useTranslation();
  const offsetId = useId();
  const modeId = useId();
  const speedId = useId();
  const zoomId = useId();
  const settleOnEnter = (e: KeyboardEvent): void => {
    if (e.key === "Enter") {
      onSettle();
    }
  };
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
      <div role="group" aria-label={t("label.reshape.title")} className="flex items-center gap-0.5">
        {RESHAPES.map(({ op, icon: Icon, labelKey }) => (
          <Button
            key={op}
            type="button"
            variant="ghost"
            size="icon-lg"
            aria-label={t(labelKey)}
            title={t(labelKey)}
            disabled={reshapeDisabled}
            onClick={() => {
              onReshape(op);
            }}
          >
            <Icon aria-hidden />
          </Button>
        ))}
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
        <label htmlFor={modeId} className="text-muted-foreground">
          {t("label.transport.scrollMode")}
        </label>
        <select
          id={modeId}
          value={scroll.kind}
          onChange={(e) => {
            const kind = SCROLL_KINDS.find((k) => k.kind === e.target.value)?.kind;
            if (kind !== undefined) {
              onScroll({ kind });
            }
            onSettle();
          }}
          className="border-input bg-background h-7 rounded-md border px-1.5 text-sm"
        >
          {SCROLL_KINDS.map(({ kind, labelKey }) => (
            <option key={kind} value={kind}>
              {t(labelKey)}
            </option>
          ))}
        </select>
        {scroll.kind === "osu" ? (
          <>
            <label htmlFor={speedId} className="text-muted-foreground">
              {t("label.transport.osuSpeed")}
            </label>
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
              onPointerUp={onSettle}
              onKeyDown={settleOnEnter}
              className="border-input bg-background h-7 w-14 rounded-md border px-1.5 font-mono text-sm tabular-nums disabled:opacity-40"
            />
          </>
        ) : (
          <>
            <label htmlFor={speedId} className="text-muted-foreground">
              {t("label.transport.scroll")}
            </label>
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
              onPointerUp={onSettle}
              className="w-24 accent-current disabled:opacity-40"
            />
            <output htmlFor={speedId} className="w-16 font-mono text-xs tabular-nums">
              {t("label.transport.scrollValue", { value: scroll.pxPerMs.toFixed(2) })}
            </output>
          </>
        )}
        <label className="flex items-center gap-1.5">
          <input
            type="checkbox"
            checked={scroll.fit}
            onChange={(e) => {
              onScroll({ fit: e.target.checked });
              onSettle();
            }}
          />
          {t("label.transport.fit")}
        </label>
      </div>
      <div className="flex items-center gap-2 text-sm">
        <label htmlFor={zoomId} className="text-muted-foreground">
          {t("label.transport.zoom")}
        </label>
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
          onPointerUp={onSettle}
          className="w-24 accent-current"
        />
        <output htmlFor={zoomId} className="w-12 font-mono text-xs tabular-nums">
          {t("label.transport.zoomValue", { value: zoom.toFixed(2) })}
        </output>
      </div>
    </div>
  );
}
