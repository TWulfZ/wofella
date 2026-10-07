// Maps the IPC skin onto the renderer's LoadedSkin, decoding images in the webview as ADR 0019 prescribes.

import type { NoteBodyStyleDto, SkinDto, SkinEffectsDto, SkinFileDto } from "@/ipc/bindings";
import { base64ToBytes } from "../decodeAudio";
import { DEFAULT_EFFECT_DRAW_PARAMS } from "../drawEffects";
import { createSurface, type Surface, type SurfaceFactory } from "../offscreen";
import { noteImage } from "../skinLayout";
import {
  type LoadedSkin,
  type NoteBodyStyle,
  SKIN_SLOT,
  type SkinEffectConfig,
  type SkinImage,
  type SkinRgba,
  type SkinSlot,
} from "../skinModel";
import { DEFAULT_STAGE_PARAMS } from "../stage";

export interface SkinLoaderParams {
  /**
   * Taller body images are cropped to this many pixels so a "long body" skin does not pin a huge texture
   * (ADR 0019). No body is ever drawn this tall at a playable height.
   */
  maxBodyHeightPx: number;
  /** skin.ini defaults for numbers the wire leaves null (research 06; the server already applies them). */
  defaultVersion: number;
  defaultColumnWidth: number;
  defaultColumnSpacing: number;
  defaultColumnLineWidth: number;
  defaultHitPosition: number;
  /** `LegacyManiaSkinConfiguration.cs` L36–38 (stable units); score and combo shared with the procedural stage. */
  defaultLightPosition: number;
  defaultScorePosition: number;
  defaultComboPosition: number;
  /**
   * The wiki's stable default for an unset `ColourLight#` (W `Skinning/skin.ini` L677–683); lazer leaves the light
   * white (`LegacyColumnBackground.cs` L38–39). Unverified against stable (research 06, open questions).
   */
  defaultLightColour: SkinRgba;
}

export const DEFAULT_SKIN_LOADER_PARAMS: SkinLoaderParams = {
  maxBodyHeightPx: 4096,
  defaultVersion: 1,
  defaultColumnWidth: DEFAULT_STAGE_PARAMS.defaultColumnWidth,
  defaultColumnSpacing: 0,
  defaultColumnLineWidth: 2,
  defaultHitPosition: DEFAULT_STAGE_PARAMS.defaultHitPosition,
  defaultLightPosition: 413,
  defaultScorePosition: DEFAULT_EFFECT_DRAW_PARAMS.defaultScorePosition,
  defaultComboPosition: DEFAULT_EFFECT_DRAW_PARAMS.defaultComboPosition,
  defaultLightColour: { r: 55, g: 255, b: 255, a: 255 },
};

export interface SkinLoadResult {
  skin: LoadedSkin;
  /** Slots the server resolved but the webview could not decode; the renderer draws them procedurally. */
  failedSlots: SkinSlot[];
  /** Closes every bitmap; idempotent. */
  dispose: () => void;
}

const SLOT_PATTERN =
  /^(?:note\.\d+(?:\.head|\.tail)?|body\.\d+|key\.\d+(?:\.down)?|stage\.(?:left|right|bottom|hint|light)|hit\.(?:300g|300|200|100|50|0)|combo\.\d|lighting\.[nl]\.\d+)$/;

const BODY_SLOT_PATTERN = /^body\.\d+$/;

const BODY_STYLES: Readonly<Record<NoteBodyStyleDto, NoteBodyStyle>> = {
  stretch: "Stretch",
  repeat_top: "RepeatTop",
  repeat_bottom: "RepeatBottom",
  repeat_top_and_bottom: "RepeatTopAndBottom",
};

function isSkinSlot(slot: string): slot is SkinSlot {
  return SLOT_PATTERN.test(slot);
}

function rgba(c: readonly [number, number, number, number] | null): SkinRgba | null {
  return c === null ? null : { r: c[0], g: c[1], b: c[2], a: c[3] };
}

function orDefault(values: readonly (number | null)[], fallback: number): number[] {
  return values.map((v) => v ?? fallback);
}

interface Decoded {
  bitmap: ImageBitmap;
  height: number;
}

// Sizes come from the header Rust checked; honouring EXIF rotation would swap them for rotated JPEGs.
const NO_ROTATION: ImageBitmapOptions = { imageOrientation: "none" };

async function decode(f: SkinFileDto, crop: boolean, maxHeight: number): Promise<Decoded> {
  const blob = new Blob([base64ToBytes(f.base64)], { type: f.mime });
  if (!crop || f.height <= maxHeight) {
    return { bitmap: await createImageBitmap(blob, NO_ROTATION), height: f.height };
  }
  // Row 0 faces the tail, so percy skins put their transparent lead-in and cap in the top rows (research 06, Geometry).
  const bitmap = await createImageBitmap(blob, 0, 0, f.width, maxHeight, NO_ROTATION);
  return { bitmap, height: maxHeight };
}

function flipVertically(img: SkinImage, surface: SurfaceFactory): Surface | null {
  const s = surface(img.width, img.height);
  if (s === null) {
    return null;
  }
  s.ctx.translate(0, img.height);
  s.ctx.scale(1, -1);
  s.ctx.drawImage(img.bitmap, 0, 0, img.width, img.height, 0, 0, img.width, img.height);
  return s;
}

/** Multiplies the light by the colour and keeps its alpha times the colour's, as lazer's sprite `Colour` does. */
function tintLight(img: SkinImage, colour: SkinRgba, surface: SurfaceFactory): Surface | null {
  const s = surface(img.width, img.height);
  if (s === null) {
    return null;
  }
  const whole = [0, 0, img.width, img.height, 0, 0, img.width, img.height] as const;
  s.ctx.drawImage(img.bitmap, ...whole);
  s.ctx.globalCompositeOperation = "multiply";
  s.ctx.fillStyle = `rgb(${colour.r}, ${colour.g}, ${colour.b})`;
  s.ctx.fillRect(0, 0, img.width, img.height);
  s.ctx.globalCompositeOperation = "destination-in";
  s.ctx.globalAlpha = colour.a / 255;
  s.ctx.drawImage(img.bitmap, ...whole);
  s.ctx.globalAlpha = 1;
  s.ctx.globalCompositeOperation = "source-over";
  return s;
}

function tintedLights(
  light: SkinImage | undefined,
  colours: readonly (SkinRgba | null)[],
  columns: number,
  fallback: SkinRgba,
  surface: SurfaceFactory,
) {
  const lights = new Map<number, SkinImage>();
  const byColour = new Map<string, Surface | null>();
  if (light === undefined) {
    return { lights, surfaces: [] };
  }
  for (let col = 0; col < columns; col++) {
    const c = colours[col] ?? fallback;
    const key = `${c.r},${c.g},${c.b},${c.a}`;
    let s = byColour.get(key);
    if (s === undefined) {
      s = tintLight(light, c, surface);
      byColour.set(key, s);
    }
    if (s !== null) {
      lights.set(col, { ...light, bitmap: s.image });
    }
  }
  return { lights, surfaces: [...byColour.values()].flatMap((s) => (s === null ? [] : [s])) };
}

function effectConfig(e: SkinEffectsDto, columns: number, params: SkinLoaderParams): SkinEffectConfig {
  const widths = (w: readonly (number | null)[]): number[] => Array.from({ length: columns }, (_, i) => w[i] ?? 0);
  return {
    scorePosition: e.scorePosition ?? params.defaultScorePosition,
    comboPosition: e.comboPosition ?? params.defaultComboPosition,
    lightingNWidth: widths(e.lightingNWidth),
    lightingLWidth: widths(e.lightingLWidth),
    comboOverlap: e.comboOverlap ?? 0,
  };
}

/** Columns without a tail image, or whose flip cannot be painted, are left out and drawn procedurally. */
function flippedTails(images: LoadedSkin["images"], columns: number, surface: SurfaceFactory) {
  const flipped = new Map<CanvasImageSource, Surface | null>();
  const tails = new Map<number, SkinImage>();
  for (let col = 0; col < columns; col++) {
    const tail = noteImage({ images }, col, "lnTail");
    if (tail === undefined) {
      continue;
    }
    let s = flipped.get(tail.bitmap);
    if (s === undefined) {
      s = flipVertically(tail, surface);
      flipped.set(tail.bitmap, s);
    }
    if (s !== null) {
      tails.set(col, { ...tail, bitmap: s.image });
    }
  }
  const surfaces = [...flipped.values()].flatMap((s) => (s === null ? [] : [s]));
  return { tails, surfaces };
}

export async function loadSkin(
  dto: SkinDto,
  params: SkinLoaderParams = DEFAULT_SKIN_LOADER_PARAMS,
  surface: SurfaceFactory = createSurface,
): Promise<SkinLoadResult> {
  const { config } = dto;
  const decodes = new Map<string, Promise<Decoded | null>>();
  const decodeOnce = (index: number, crop: boolean): Promise<Decoded | null> => {
    const f = dto.files[index];
    if (f === undefined) {
      return Promise.resolve(null);
    }
    const key = `${index}:${crop ? "body" : "whole"}`;
    let pending = decodes.get(key);
    if (pending === undefined) {
      pending = decode(f, crop, params.maxBodyHeightPx).catch(() => null);
      decodes.set(key, pending);
    }
    return pending;
  };

  const refs = dto.images.filter((ref) => isSkinSlot(ref.slot));
  const resolved = await Promise.all(
    refs.map(async (ref) => ({ ref, decoded: await decodeOnce(ref.file, BODY_SLOT_PATTERN.test(ref.slot)) })),
  );

  const images = new Map<SkinSlot, SkinImage>();
  const failedSlots: SkinSlot[] = [];
  for (const { ref, decoded } of resolved) {
    const slot = ref.slot as SkinSlot;
    const f = dto.files[ref.file];
    if (decoded === null || f === undefined) {
      failedSlots.push(slot);
      continue;
    }
    images.set(slot, {
      bitmap: decoded.bitmap,
      width: f.width,
      height: decoded.height,
      scale: f.scale,
      sourceHeight: f.height,
    });
  }

  const bitmaps = (await Promise.all(decodes.values())).flatMap((d) => (d === null ? [] : [d.bitmap]));
  const skin = {
    version: dto.version ?? params.defaultVersion,
    columnWidth: orDefault(config.columnWidth, params.defaultColumnWidth),
    columnSpacing: orDefault(config.columnSpacing, params.defaultColumnSpacing),
    columnLineWidth: orDefault(config.columnLineWidth, params.defaultColumnLineWidth),
    hitPosition: config.hitPosition ?? params.defaultHitPosition,
    lightPosition: config.lightPosition ?? params.defaultLightPosition,
    widthForNoteHeightScale: config.widthForNoteHeightScale,
    noteBodyStyle: BODY_STYLES[config.noteBodyStyle],
    judgementLine: config.judgementLine,
    keysUnderNotes: config.keysUnderNotes,
    colours: {
      column: config.colours.column.map(rgba),
      columnLine: rgba(config.colours.columnLine),
      judgementLine: rgba(config.colours.judgementLine),
    },
    images,
  };
  const columns = skin.columnWidth.length;
  const { tails, surfaces: tailSurfaces } = flippedTails(images, columns, surface);
  const lightColours = dto.effects.lightColours.map(rgba);
  const lights = tintedLights(images.get(SKIN_SLOT.stageLight), lightColours, columns, params.defaultLightColour, surface);
  const surfaces = [...tailSurfaces, ...lights.surfaces];
  let disposed = false;
  return {
    skin: { ...skin, lnTails: tails, effects: effectConfig(dto.effects, columns, params), stageLights: lights.lights },
    failedSlots,
    dispose: () => {
      if (disposed) {
        return;
      }
      disposed = true;
      for (const bitmap of bitmaps) {
        bitmap.close();
      }
      for (const s of surfaces) {
        s.release();
      }
    },
  };
}
