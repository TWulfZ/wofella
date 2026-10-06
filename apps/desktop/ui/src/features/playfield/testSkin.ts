// Synthetic skins for renderer tests; no user skin or ppy asset is ever used (ADR 0019).

import type { LoadedSkin, SkinImage, SkinSlot } from "./skinModel";

export function image(width: number, height: number, scale = 1): SkinImage {
  return { bitmap: { width, height } as unknown as CanvasImageSource, width, height, scale };
}

export function skin7k(overrides: Partial<LoadedSkin> = {}, images: [SkinSlot, SkinImage][] = []): LoadedSkin {
  return {
    version: 2.7,
    columnWidth: [30, 30, 30, 40, 30, 30, 30],
    columnSpacing: [0, 0, 5, 5, 0, 0],
    columnLineWidth: [2, 2, 2, 2, 2, 2, 2, 2],
    hitPosition: 428,
    widthForNoteHeightScale: 42,
    noteBodyStyle: "RepeatBottom",
    judgementLine: true,
    keysUnderNotes: false,
    colours: { column: [], columnLine: null, judgementLine: null },
    images: new Map(images),
    ...overrides,
  };
}
