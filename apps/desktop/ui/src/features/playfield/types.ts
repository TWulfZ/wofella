// Structural mirror of the Rust ChartWindowDto (T1), kept local so projection does not depend on generated bindings;
// the IPC layer maps onto it.

export type ColumnHand = "left" | "right" | "both";

export interface ChartNote {
  tMs: number;
  /** 0-based, column 0 leftmost (the chart model's convention). */
  col: number;
  /** Set for LNs only. */
  endMs: number | null;
}

export interface TimingLine {
  tMs: number;
  kind: "red" | "green";
  /** Red lines only. */
  beatLenMs: number | null;
  /** Red lines only. */
  meter: number | null;
  /** Green lines only. */
  sv: number | null;
}

export interface ChartWindow {
  md5: string;
  keymode: number;
  fromMs: number;
  toMs: number;
  notes: ChartNote[];
  timing: TimingLine[];
  layout: { id: string; columns: { hand: ColumnHand; finger: string }[] };
  chartSpan: { firstMs: number; endMs: number };
  audioFilename: string | null;
}
