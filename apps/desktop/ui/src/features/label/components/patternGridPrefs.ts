// Per-viewer convenience: a blocked, cleared or corrupt storage must leave every axis on its default (collapsed).

export const PATTERN_GRID_PREFS = {
  openAxesKey: "wolluf.label.patternGrid.openAxes",
} as const;

export function readOpenAxes(): ReadonlySet<string> {
  try {
    const raw = globalThis.localStorage.getItem(PATTERN_GRID_PREFS.openAxesKey);
    const parsed: unknown = raw === null ? [] : JSON.parse(raw);
    return new Set(Array.isArray(parsed) ? parsed.filter((axis): axis is string => typeof axis === "string") : []);
  } catch {
    return new Set();
  }
}

export function writeOpenAxes(axes: Iterable<string>): void {
  try {
    globalThis.localStorage.setItem(PATTERN_GRID_PREFS.openAxesKey, JSON.stringify([...axes]));
  } catch {
    // Not persisted; the in-memory choice still applies for this session.
  }
}
