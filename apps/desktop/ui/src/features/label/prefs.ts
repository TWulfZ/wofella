// Per-viewer conveniences only: a blocked or cleared storage must leave the screen working on defaults.

export type ScrollPref = number | "fit";

export const LABEL_PREFS = {
  offsetKey: "wolluf.label.audioOffsetMs",
  scrollKey: "wolluf.label.scroll",
  minOffsetMs: -100,
  maxOffsetMs: 100,
  minPxPerMs: 0.2,
  maxPxPerMs: 3,
} as const;

const FIT = "fit";

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

export function readScroll(): ScrollPref {
  const raw = read(LABEL_PREFS.scrollKey);
  const n = parseNumber(raw);
  return n === null ? FIT : clamp(n, LABEL_PREFS.minPxPerMs, LABEL_PREFS.maxPxPerMs);
}

export function writeScroll(scroll: ScrollPref): void {
  write(LABEL_PREFS.scrollKey, String(scroll));
}
