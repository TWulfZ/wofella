// Per-viewer conveniences only: a blocked or cleared storage must leave the screen working on defaults.

import {
  clampOsuSpeed,
  clampZoom,
  DEFAULT_PLAYFIELD_EFFECTS,
  DEFAULT_STAGE_PARAMS,
  type PlayfieldEffects,
  type ScrollMode,
} from "@/features/playfield";
import { SKIN_CHOICE_KEY } from "@/features/preferences";

// Settings owns the default skin; the screen's picker writes the same key, so both always agree.
export { readSkinChoice, type SkinChoice, writeSkinChoice } from "@/features/preferences";

export type ScrollKind = ScrollMode["kind"];

export interface ScrollPrefs {
  kind: ScrollKind;
  osuSpeed: number;
  pxPerMs: number;
}

export interface ScrollDefaults {
  osuSpeed: number;
  pxPerMs: number;
}

export const LABEL_PREFS = {
  offsetKey: "wolluf.label.audioOffsetMs",
  // Each field has its own key so writing one never freezes the defaults of the others.
  scrollKindKey: "wolluf.label.scrollKind",
  osuSpeedKey: "wolluf.label.osuSpeed",
  pxPerMsKey: "wolluf.label.pxPerMs",
  zoomKey: "wolluf.label.zoom",
  /** Pixels, clamped on use: the screen it was stored on may have been wider. */
  panelWidthKey: "wolluf.label.panelWidthPx",
  skinKey: SKIN_CHOICE_KEY,
  playbackRateKey: "wolluf.label.playbackRate",
  effectKeys: {
    percy: "wolluf.label.effect.percy",
    judgements: "wolluf.label.effect.judgements",
    combo: "wolluf.label.effect.combo",
    keyPress: "wolluf.label.effect.keyPress",
    lighting: "wolluf.label.effect.lighting",
  } satisfies Record<keyof PlayfieldEffects, string>,
  settingsHintSeenKey: "wolluf.label.settingsHintSeen",
  /** Before scroll modes it held a px/ms number or "fit". */
  legacyScrollKey: "wolluf.label.scroll",
  minOffsetMs: -100,
  maxOffsetMs: 100,
  minPxPerMs: 0.2,
  maxPxPerMs: 3,
  minPlaybackRate: 0.5,
  maxPlaybackRate: 1.5,
  playbackRateStep: 0.05,
} as const;

const SCROLL_KINDS: readonly ScrollKind[] = ["osu", "pxPerMs"];

function read(key: string): string | null {
  try {
    return globalThis.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    globalThis.localStorage.setItem(key, value);
  } catch {
    // Not persisted; the in-memory value still applies for this session.
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function parseNumber(raw: string | null): number | null {
  if (raw === null || raw.trim() === "") {
    return null;
  }
  const n = Number(raw);
  return Number.isFinite(n) ? n : null;
}

export function readOffsetMs(): number {
  const n = parseNumber(read(LABEL_PREFS.offsetKey));
  return n === null ? 0 : clamp(Math.round(n), LABEL_PREFS.minOffsetMs, LABEL_PREFS.maxOffsetMs);
}

export function writeOffsetMs(offsetMs: number): void {
  write(LABEL_PREFS.offsetKey, String(offsetMs));
}

export function readScrollPrefs(defaults: ScrollDefaults): ScrollPrefs {
  const rawKind = read(LABEL_PREFS.scrollKindKey);
  const kind = SCROLL_KINDS.find((k) => k === rawKind) ?? "osu";
  const osuSpeed = parseNumber(read(LABEL_PREFS.osuSpeedKey)) ?? defaults.osuSpeed;
  const pxPerMs =
    parseNumber(read(LABEL_PREFS.pxPerMsKey)) ?? parseNumber(read(LABEL_PREFS.legacyScrollKey)) ?? defaults.pxPerMs;
  return {
    kind,
    osuSpeed: clampOsuSpeed(osuSpeed),
    pxPerMs: clamp(pxPerMs, LABEL_PREFS.minPxPerMs, LABEL_PREFS.maxPxPerMs),
  };
}

/** False until the player picks a speed here; until then the cfg ManiaSpeed default applies. */
export function hasStoredOsuSpeed(): boolean {
  return parseNumber(read(LABEL_PREFS.osuSpeedKey)) !== null;
}

export function writeScrollKind(kind: ScrollKind): void {
  write(LABEL_PREFS.scrollKindKey, kind);
}

export function writeOsuSpeed(speed: number): void {
  write(LABEL_PREFS.osuSpeedKey, String(speed));
}

export function writePxPerMs(pxPerMs: number): void {
  write(LABEL_PREFS.pxPerMsKey, String(pxPerMs));
}

export function scrollFromPrefs(prefs: ScrollPrefs): ScrollMode {
  return prefs.kind === "osu" ? { kind: "osu", speed: prefs.osuSpeed } : { kind: "pxPerMs", value: prefs.pxPerMs };
}

export function readZoom(): number {
  const n = parseNumber(read(LABEL_PREFS.zoomKey));
  return n === null ? DEFAULT_STAGE_PARAMS.defaultZoom : clampZoom(n);
}

export function writeZoom(zoom: number): void {
  write(LABEL_PREFS.zoomKey, String(zoom));
}

/** Snapped to the slider's step, so a hand-edited value still lands on a position the slider can show. */
export function clampPlaybackRate(rate: number): number {
  const steps = Math.round(rate / LABEL_PREFS.playbackRateStep);
  const snapped = Math.round(steps * LABEL_PREFS.playbackRateStep * 100) / 100;
  return clamp(snapped, LABEL_PREFS.minPlaybackRate, LABEL_PREFS.maxPlaybackRate);
}

export function readPlaybackRate(): number {
  const n = parseNumber(read(LABEL_PREFS.playbackRateKey));
  return n === null ? 1 : clampPlaybackRate(n);
}

export function writePlaybackRate(rate: number): void {
  write(LABEL_PREFS.playbackRateKey, String(rate));
}

export function readPanelWidthPx(fallback: number): number {
  const n = parseNumber(read(LABEL_PREFS.panelWidthKey));
  return n === null || n <= 0 ? fallback : n;
}

export function writePanelWidthPx(widthPx: number): void {
  write(LABEL_PREFS.panelWidthKey, String(Math.round(widthPx)));
}

const EFFECTS = Object.keys(LABEL_PREFS.effectKeys) as (keyof PlayfieldEffects)[];

export function readPlayfieldEffects(): PlayfieldEffects {
  const effects = { ...DEFAULT_PLAYFIELD_EFFECTS };
  for (const effect of EFFECTS) {
    const raw = read(LABEL_PREFS.effectKeys[effect]);
    if (raw === "true" || raw === "false") {
      effects[effect] = raw === "true";
    }
  }
  return effects;
}

export function writePlayfieldEffects(effects: PlayfieldEffects): void {
  for (const effect of EFFECTS) {
    write(LABEL_PREFS.effectKeys[effect], String(effects[effect]));
  }
}

// A stored playback setting means the viewer already found where these live, before the nudge existed.
const PLAYBACK_KEYS: readonly string[] = [
  LABEL_PREFS.offsetKey,
  LABEL_PREFS.scrollKindKey,
  LABEL_PREFS.osuSpeedKey,
  LABEL_PREFS.pxPerMsKey,
  LABEL_PREFS.legacyScrollKey,
  LABEL_PREFS.zoomKey,
  LABEL_PREFS.playbackRateKey,
  LABEL_PREFS.skinKey,
  ...Object.values(LABEL_PREFS.effectKeys),
];

export function readSettingsHintSeen(): boolean {
  return read(LABEL_PREFS.settingsHintSeenKey) === "true" || PLAYBACK_KEYS.some((key) => read(key) !== null);
}

export function writeSettingsHintSeen(): void {
  write(LABEL_PREFS.settingsHintSeenKey, "true");
}
