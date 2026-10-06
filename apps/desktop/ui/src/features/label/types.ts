// Structural mirrors of the labeling DTOs (crates/app/src/features/labeling/dto.rs), kept local so the pure modules do
// not depend on generated bindings; the IPC layer maps onto them.

export type ThumbSide = "left" | "right";

export type WindowOp = "widen" | "narrow" | "next" | "prev";

export interface Anchor {
  md5: string;
  t0Ms: number;
  t1Ms: number;
  /** 1-based, column 1 leftmost, ascending. */
  cols: number[];
}

export interface LabelWindow {
  anchor: Anchor;
  title: string;
  artist: string;
  version: string;
  level: string | null;
  stratum: string;
  played: boolean;
}
