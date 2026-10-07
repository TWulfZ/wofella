// Synthetic skins for renderer tests; no user skin or ppy asset is ever used (ADR 0019).

import { noteImage } from "./skinLayout";
import type { LoadedSkin, SkinImage, SkinSlot } from "./skinModel";

/** `sourceHeight` above `height` stands for a body the loader cropped. */
export function image(width: number, height: number, scale = 1, sourceHeight = height): SkinImage {
  return { bitmap: { width, height } as unknown as CanvasImageSource, width, height, scale, sourceHeight };
}

/** What the loader's flip yields: same size, a distinct bitmap standing for the vertically flipped pixels. */
function flippedTails(skin: LoadedSkin): Map<number, SkinImage> {
  const tails = new Map<number, SkinImage>();
  for (const col of skin.columnWidth.keys()) {
    const tail = noteImage(skin, col, "lnTail");
    if (tail !== undefined) {
      tails.set(col, { ...tail, bitmap: { flippedFrom: tail.bitmap } as unknown as CanvasImageSource });
    }
  }
  return tails;
}

export function skin7k(overrides: Partial<LoadedSkin> = {}, images: [SkinSlot, SkinImage][] = []): LoadedSkin {
  const skin: LoadedSkin = {
    version: 2.7,
    columnWidth: [30, 30, 30, 40, 30, 30, 30],
    columnSpacing: [0, 0, 5, 5, 0, 0],
    columnLineWidth: [2, 2, 2, 2, 2, 2, 2, 2],
    hitPosition: 428,
    lightPosition: 413,
    widthForNoteHeightScale: 42,
    noteBodyStyle: "RepeatBottom",
    judgementLine: true,
    keysUnderNotes: false,
    colours: { column: [], columnLine: null, judgementLine: null },
    images: new Map(images),
    lnTails: new Map(),
    effects: {
      scorePosition: 300,
      comboPosition: 111,
      lightingNWidth: [0, 0, 0, 0, 0, 0, 0],
      lightingLWidth: [0, 0, 0, 0, 0, 0, 0],
      comboOverlap: 0,
    },
    stageLights: new Map(),
    ...overrides,
  };
  return "lnTails" in overrides ? skin : { ...skin, lnTails: flippedTails(skin) };
}
