// Per-viewer conveniences only: a blocked or cleared storage must leave the screen working on defaults.

import { clampOsuSpeed, clampZoom, DEFAULT_STAGE_PARAMS, type ScrollMode } from "@/features/playfield";

export type ScrollKind = ScrollMode["kind"];

export interface ScrollPrefs {
  kind: ScrollKind;
  osuSpeed: number;
  pxPerMs: number;
  fit: boolean;
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
  fitKey: "wolluf.label.fit",
  zoomKey: "wolluf.label.zoom",
  /** JSON so a skin folder literally named like a sentinel cannot be mistaken for "None". */
  skinKey: "wolluf.label.skin",
  /** Before scroll modes it held a px/ms number or "fit". */
  legacyScrollKey: "wolluf.label.scroll",
  minOffsetMs: -100,
  maxOffsetMs: 100,
  minPxPerMs: 0.2,
  maxPxPerMs: 3,
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
    fit: read(LABEL_PREFS.fitKey) === "true",
  };
}

/** False until the player picks a speed here; until then the cfg ManiaSpeed default applies. */
export function hasStoredOsuSpeed(): boolean {
  return parseNumber(read(LABEL_PREFS.osuSpeedKey)) !== null;
}

/** `folder: null` is the procedural look; no stored choice follows the cfg's active skin. */
export interface SkinChoice {
  folder: string | null;
}

export function readSkinChoice(): SkinChoice | undefined {
  const raw = read(LABEL_PREFS.skinKey);
  if (raw === null) {
    return undefined;
  }
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed === "object" && parsed !== null && "folder" in parsed) {
      const { folder } = parsed;
      if (folder === null || typeof folder === "string") {
        return { folder };
      }
    }
  } catch {
    // Unreadable: treated as never chosen.
  }
  return undefined;
}

export function writeSkinChoice(choice: SkinChoice): void {
  write(LABEL_PREFS.skinKey, JSON.stringify({ folder: choice.folder }));
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

export function writeFit(fit: boolean): void {
  write(LABEL_PREFS.fitKey, String(fit));
}

export function scrollFromPrefs(prefs: ScrollPrefs): ScrollMode | "fit" {
  if (prefs.fit) {
    return "fit";
  }
  return prefs.kind === "osu" ? { kind: "osu", speed: prefs.osuSpeed } : { kind: "pxPerMs", value: prefs.pxPerMs };
}

export function readZoom(): number {
  const n = parseNumber(read(LABEL_PREFS.zoomKey));
  return n === null ? DEFAULT_STAGE_PARAMS.defaultZoom : clampZoom(n);
}

export function writeZoom(zoom: number): void {
  write(LABEL_PREFS.zoomKey, String(zoom));
}
