// Per-viewer convenience: a blocked or cleared storage must leave the grid on its default density.

export const PATTERN_GRID_PREFS = {
  alwaysCollapsedKey: "wolluf.label.patternGrid.alwaysCollapsed",
} as const;

export function readAlwaysCollapsed(): boolean {
  try {
    return globalThis.localStorage.getItem(PATTERN_GRID_PREFS.alwaysCollapsedKey) === "true";
  } catch {
    return false;
  }
}

export function writeAlwaysCollapsed(collapsed: boolean): void {
  try {
    globalThis.localStorage.setItem(PATTERN_GRID_PREFS.alwaysCollapsedKey, String(collapsed));
  } catch {
    // Not persisted; the in-memory choice still applies for this session.
  }
}
