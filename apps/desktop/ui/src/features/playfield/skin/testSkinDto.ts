// Synthetic skin DTOs and a fake createImageBitmap for loader tests; no user skin or ppy asset is used (ADR 0019).

import { vi } from "vitest";
import type { ManiaConfigDto, SkinDto, SkinEffectsDto, SkinFileDto } from "@/ipc/bindings";

/** A file whose decode the fake rejects, standing in for a corrupt PNG the header check let through. */
export const BROKEN_MIME = "image/x-broken";

export interface FakeBitmap {
  width: number;
  height: number;
  /** The source rectangle the loader asked for, or null for the whole image. */
  crop: [number, number, number, number] | null;
  close: ReturnType<typeof vi.fn>;
}

export function fakeCreateImageBitmap() {
  const bitmaps: FakeBitmap[] = [];
  const fn = vi.fn(async (blob: Blob, sx?: number, sy?: number, sw?: number, sh?: number): Promise<FakeBitmap> => {
    if (blob.type === BROKEN_MIME) {
      return Promise.reject(new DOMException("undecodable", "InvalidStateError"));
    }
    const crop: FakeBitmap["crop"] =
      sx === undefined || sy === undefined || sw === undefined || sh === undefined ? null : [sx, sy, sw, sh];
    const bitmap: FakeBitmap = { width: sw ?? 1, height: sh ?? 1, crop, close: vi.fn() };
    bitmaps.push(bitmap);
    return Promise.resolve(bitmap);
  });
  return { fn, bitmaps };
}

export function file(width: number, height: number, overrides: Partial<SkinFileDto> = {}): SkinFileDto {
  return { mime: "image/png", scale: 1, width, height, base64: "iVBORw0KGgo=", ...overrides };
}

export function config7k(overrides: Partial<ManiaConfigDto> = {}): ManiaConfigDto {
  return {
    keys: 7,
    columnWidth: [40, 42, 42, 42, 42, 42, 42],
    columnSpacing: [0, 0, 0, 0, 0, 0],
    columnLineWidth: [2, 2, 2, 2, 2, 2, 2, 2],
    hitPosition: 428,
    lightPosition: 413,
    widthForNoteHeightScale: 42,
    noteBodyStyle: "repeat_bottom",
    judgementLine: true,
    keysUnderNotes: false,
    upsideDown: false,
    barlineHeight: 1.2,
    colours: { column: [], columnLine: null, judgementLine: null, barline: null, hold: null },
    ...overrides,
  };
}

/** What the server sends for a skin.ini without effect keys: lazer's defaults already applied. */
export function skinEffects(keys = 7, overrides: Partial<SkinEffectsDto> = {}): SkinEffectsDto {
  return {
    scorePosition: 300,
    comboPosition: 111,
    lightingNWidth: Array.from({ length: keys }, () => 0),
    lightingLWidth: Array.from({ length: keys }, () => 0),
    lightColours: Array.from({ length: keys }, () => null),
    comboOverlap: 0,
    ...overrides,
  };
}

export function skinDto(overrides: Partial<SkinDto> = {}): SkinDto {
  return {
    folder: "Pilot Skin",
    name: "Pilot",
    version: 2.5,
    config: config7k(),
    effects: skinEffects(),
    images: [],
    files: [],
    diagnostics: [],
    ...overrides,
  };
}
