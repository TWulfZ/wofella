import type { ChartWindow, ColumnHand, TimingLine } from "./types";

export interface PlayfieldParams {
  noteHeightPx: number;
  lnTailHeightPx: number;
  /** Used when a red line carries no meter (osu! defaults to 4/4). */
  defaultMeter: number;
  /** Guards against pathological beat lengths producing millions of lines. */
  maxBeatLines: number;
  /** Floors for fitting an empty window or a tiny view, so the fitted speed stays finite and positive. */
  minFitSpanMs: number;
  minFitRoomPx: number;
}

export const DEFAULT_PLAYFIELD_PARAMS: PlayfieldParams = {
  noteHeightPx: 12,
  lnTailHeightPx: 6,
  defaultMeter: 4,
  maxBeatLines: 512,
  minFitSpanMs: 1,
  minFitRoomPx: 1,
};

export interface ProjectView {
  /** Chart time at the judgement line. */
  nowMs: number;
  /** Scroll speed. */
  pxPerMs: number;
  width: number;
  height: number;
  /** Judgement line, in px from the top. */
  judgeY: number;
}

export type NoteKind = "tap" | "lnHead" | "lnBody" | "lnTail";

export interface NoteRect {
  col: number;
  x: number;
  y: number;
  w: number;
  h: number;
  kind: NoteKind;
  /** Cut to the visible area. */
  clipped: boolean;
}

/** Unclipped y of a note's head and LN tail; the skinned path sizes sprites itself, so it cannot use NoteRect. */
export interface NoteSpan {
  col: number;
  headY: number;
  tailY: number | null;
}

export interface Projection {
  /** In draw order: an LN's body comes before its tail and head. */
  notes: NoteRect[];
  spans: NoteSpan[];
  beatLines: { y: number; measure: boolean }[];
  handSeparators: { x: number }[];
  columns: { x: number; w: number; hand: ColumnHand }[];
  /** Visible regions outside [fromMs, toMs], top to bottom. */
  shade: { y0: number; y1: number }[];
}

export function project(
  window: ChartWindow,
  view: ProjectView,
  params: PlayfieldParams = DEFAULT_PLAYFIELD_PARAMS,
): Projection {
  const { nowMs, pxPerMs, width, height, judgeY } = view;
  // Later notes sit higher, scrolling down toward the judgement line as in osu!mania; SV is ignored on purpose.
  const yOf = (tMs: number): number => judgeY - (tMs - nowMs) * pxPerMs;
  const topMs = nowMs + judgeY / pxPerMs;
  const bottomMs = nowMs - (height - judgeY) / pxPerMs;

  const colW = width / window.layout.columns.length;
  const columns = window.layout.columns.map(({ hand }, i) => ({ x: i * colW, w: colW, hand }));
  const handSeparators: { x: number }[] = [];
  for (let i = 1; i < columns.length; i++) {
    const prev = columns[i - 1];
    const cur = columns[i];
    if (prev !== undefined && cur !== undefined && prev.hand !== cur.hand) {
      handSeparators.push({ x: cur.x });
    }
  }

  const notes: NoteRect[] = [];
  const spans: NoteSpan[] = [];
  const push = (col: number, top: number, bottom: number, kind: NoteKind): void => {
    const column = columns[col];
    const y0 = Math.max(top, 0);
    const y1 = Math.min(bottom, height);
    if (column === undefined || y1 <= y0) {
      return;
    }
    notes.push({ col, x: column.x, y: y0, w: column.w, h: y1 - y0, kind, clipped: y0 !== top || y1 !== bottom });
  };
  for (const note of window.notes) {
    const headY = yOf(note.tMs);
    // Every sprite of a note lies above its head, so a head at or above the top edge leaves nothing visible.
    if (columns[note.col] !== undefined && headY > 0) {
      spans.push({ col: note.col, headY, tailY: note.endMs === null ? null : yOf(note.endMs) });
    }
    if (note.endMs === null) {
      push(note.col, headY - params.noteHeightPx, headY, "tap");
      continue;
    }
    const tailY = yOf(note.endMs);
    push(note.col, tailY, headY, "lnBody");
    push(note.col, tailY - params.lnTailHeightPx, tailY, "lnTail");
    push(note.col, headY - params.noteHeightPx, headY, "lnHead");
  }

  return {
    notes,
    spans,
    beatLines: beatLines(window.timing, bottomMs, topMs, params).map(({ tMs, measure }) => ({ y: yOf(tMs), measure })),
    handSeparators,
    columns,
    shade: shade(window, yOf, height),
  };
}

function beatLines(
  timing: TimingLine[],
  fromMs: number,
  toMs: number,
  params: PlayfieldParams,
): { tMs: number; measure: boolean }[] {
  const reds = timing.filter((t) => t.kind === "red").sort((a, b) => a.tMs - b.tMs);
  const lines: { tMs: number; measure: boolean }[] = [];
  for (const [i, red] of reds.entries()) {
    const beatLen = red.beatLenMs;
    if (beatLen === null || !Number.isFinite(beatLen) || beatLen <= 0) {
      continue;
    }
    // A red line resets the beat grid, so each one owns the time until the next.
    const endMs = reds[i + 1]?.tMs ?? Infinity;
    const meter = red.meter !== null && red.meter > 0 ? red.meter : params.defaultMeter;
    // Times come from the index, not from accumulation, so float error does not drift the grid.
    for (let k = Math.max(0, Math.ceil((fromMs - red.tMs) / beatLen)); ; k++) {
      const tMs = red.tMs + k * beatLen;
      if (tMs >= endMs || tMs > toMs) {
        break;
      }
      if (lines.length >= params.maxBeatLines) {
        return lines;
      }
      lines.push({ tMs, measure: k % meter === 0 });
    }
  }
  return lines;
}

function shade(window: ChartWindow, yOf: (tMs: number) => number, height: number): { y0: number; y1: number }[] {
  const regions: { y0: number; y1: number }[] = [];
  const afterY = Math.min(yOf(window.toMs), height);
  if (afterY > 0) {
    regions.push({ y0: 0, y1: afterY });
  }
  const beforeY = Math.max(yOf(window.fromMs), 0);
  if (beforeY < height) {
    regions.push({ y0: beforeY, y1: height });
  }
  return regions;
}

/** Scroll speed that fits [fromMs, toMs] above the judgement line when paused at fromMs, last note head included. */
export function fitPxPerMs(
  window: ChartWindow,
  height: number,
  judgeY: number,
  params: PlayfieldParams = DEFAULT_PLAYFIELD_PARAMS,
): number {
  const span = Math.max(window.toMs - window.fromMs, params.minFitSpanMs);
  const room = Math.max(Math.min(judgeY, height) - params.noteHeightPx, params.minFitRoomPx);
  return room / span;
}
