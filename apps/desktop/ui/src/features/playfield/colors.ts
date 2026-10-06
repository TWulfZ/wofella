// Literal colours until feat/osu-theme merges; then these switch to its --osu-* tokens.

/** osu!mania default-skin style: outer and inner columns alternate from each edge, the centre column stands out. */
export const COLUMN_COLORS = {
  outer: "#e8ecf2",
  inner: "#5fb4ff",
  centre: "#ffcc22",
} as const;

export const PLAYFIELD_COLORS = {
  background: "#0b0d12",
  columnTint: "rgba(255, 255, 255, 0.03)",
  columnSeparator: "rgba(255, 255, 255, 0.08)",
  lnBodyAlpha: 0.55,
  beatLine: "rgba(255, 255, 255, 0.12)",
  measureLine: "rgba(255, 255, 255, 0.35)",
  handSeparator: "rgba(255, 102, 170, 0.6)",
  judgementLine: "#ff66aa",
  shade: "rgba(0, 0, 0, 0.6)",
} as const;

/** `col` is 0-based. */
export function columnColor(keymode: number, col: number): string {
  if (keymode % 2 === 1 && col === (keymode - 1) / 2) {
    return COLUMN_COLORS.centre;
  }
  const fromEdge = Math.min(col, keymode - 1 - col);
  return fromEdge % 2 === 0 ? COLUMN_COLORS.outer : COLUMN_COLORS.inner;
}
