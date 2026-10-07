// Structural shape of a loaded skin as the renderer needs it, kept local so drawing does not depend on generated
// bindings; the skin loader maps the IPC DTO onto it.

export type NoteBodyStyle = "Stretch" | "RepeatTop" | "RepeatBottom" | "RepeatTopAndBottom";

/** Channels 0–255, as written in skin.ini; alpha stays raw because stable applies it differently per element. */
export interface SkinRgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

export interface SkinColours {
  /** `Colour1` is `column[0]`. */
  column: readonly (SkinRgba | null)[];
  columnLine: SkinRgba | null;
  judgementLine: SkinRgba | null;
}

export interface SkinImage {
  bitmap: CanvasImageSource;
  /** Pixel size of `bitmap`, after any crop the loader applied. */
  width: number;
  height: number;
  /** 2 for an `@2x` file: display size is pixel size ÷ scale (research 06, "Image resolution"). */
  scale: number;
  /**
   * Pixel height of the file. Above `height` when the loader kept only the top rows: the art continues past `height`,
   * so it is never re-tiled, and Stretch maps the kept rows to their share of the hold.
   */
  sourceHeight: number;
}

/** Judgement names as the skin files spell them: `mania-hit300g` is MAX. */
export type HitResultName = "300g" | "300" | "200" | "100" | "50" | "0";

export type SkinSlot =
  | `note.${number}`
  | `note.${number}.head`
  | `note.${number}.tail`
  | `body.${number}`
  | `key.${number}`
  | `key.${number}.down`
  | "stage.left"
  | "stage.right"
  | "stage.bottom"
  | "stage.hint"
  | "stage.light"
  | `hit.${HitResultName}`
  | `combo.${number}`
  | `lighting.n.${number}`
  | `lighting.l.${number}`;

/** Column indices are 0-based, like `NoteImage0`. */
export const SKIN_SLOT = {
  note: (col: number): SkinSlot => `note.${col}`,
  head: (col: number): SkinSlot => `note.${col}.head`,
  tail: (col: number): SkinSlot => `note.${col}.tail`,
  body: (col: number): SkinSlot => `body.${col}`,
  key: (col: number): SkinSlot => `key.${col}`,
  keyDown: (col: number): SkinSlot => `key.${col}.down`,
  stageLeft: "stage.left",
  stageRight: "stage.right",
  stageBottom: "stage.bottom",
  stageHint: "stage.hint",
  stageLight: "stage.light",
  hit: (result: HitResultName): SkinSlot => `hit.${result}`,
  comboDigit: (digit: number): SkinSlot => `combo.${digit}`,
  /** Frames are 0-based; a still image is frame 0. */
  lightingN: (frame: number): SkinSlot => `lighting.n.${frame}`,
  lightingL: (frame: number): SkinSlot => `lighting.l.${frame}`,
} as const satisfies Record<string, SkinSlot | ((arg: never) => SkinSlot)>;

/** Placement of the playback effects, in stable's 480-high space with lazer's defaults (research 06). */
export interface SkinEffectConfig {
  /** Hit-burst centre, y from the top. */
  scorePosition: number;
  /** Combo counter centre, y from the top. */
  comboPosition: number;
  /** Per column; 0 means the column width. */
  lightingNWidth: readonly number[];
  lightingLWidth: readonly number[];
  /** Px the combo digits overlap; negative adds a gap. */
  comboOverlap: number;
}

/** Lengths are in stable's 480-high space, as skin.ini writes them; scaling is the layout's job. */
export interface LoadedSkin {
  /** `[General] Version`; `latest` already mapped to 2.7. */
  version: number;
  columnWidth: readonly number[];
  /** `keys - 1` gaps. */
  columnSpacing: readonly number[];
  /** `keys + 1` lines. */
  columnLineWidth: readonly number[];
  hitPosition: number;
  /** Bottom of the stage light, y from the top. */
  lightPosition: number;
  widthForNoteHeightScale: number | null;
  noteBodyStyle: NoteBodyStyle;
  judgementLine: boolean;
  keysUnderNotes: boolean;
  colours: SkinColours;
  images: ReadonlyMap<SkinSlot, SkinImage>;
  /**
   * Per column, the LN tail after lazer's chain (T, then H, then note), already flipped vertically because lazer
   * inverts the tail (`LegacyHoldNoteTailPiece.cs` L49); flipping once at load spares a transform per tail per frame.
   */
  lnTails: ReadonlyMap<number, SkinImage>;
  effects: SkinEffectConfig;
  /**
   * Per column, `stage.light` tinted with that column's `ColourLight`, painted once at load like `lnTails`; a column
   * missing here draws the untinted image.
   */
  stageLights: ReadonlyMap<number, SkinImage>;
}
