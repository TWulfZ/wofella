// Maps the IPC skin onto the renderer's LoadedSkin, decoding images in the webview as ADR 0019 prescribes.

import type { NoteBodyStyleDto, SkinDto, SkinFileDto } from "@/ipc/bindings";
import { base64ToBytes } from "../decodeAudio";
import type { LoadedSkin, NoteBodyStyle, SkinImage, SkinRgba, SkinSlot } from "../skinModel";
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
}

export const DEFAULT_SKIN_LOADER_PARAMS: SkinLoaderParams = {
  maxBodyHeightPx: 4096,
  defaultVersion: 1,
  defaultColumnWidth: DEFAULT_STAGE_PARAMS.defaultColumnWidth,
  defaultColumnSpacing: 0,
  defaultColumnLineWidth: 2,
  defaultHitPosition: DEFAULT_STAGE_PARAMS.defaultHitPosition,
};

export interface SkinLoadResult {
  skin: LoadedSkin;
  /** Slots the server resolved but the webview could not decode; the renderer draws them procedurally. */
  failedSlots: SkinSlot[];
  /** Closes every bitmap; idempotent. */
  dispose: () => void;
}

const SLOT_PATTERN = /^(?:note\.\d+(?:\.head|\.tail)?|body\.\d+|key\.\d+(?:\.down)?|stage\.(?:left|right|bottom|hint|light))$/;

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
  // The head end sits on the judgement line and is what stays on screen longest; which end stable keeps is
  // unverified (research 06).
  const bitmap = await createImageBitmap(blob, 0, f.height - maxHeight, f.width, maxHeight, NO_ROTATION);
  return { bitmap, height: maxHeight };
}

export async function loadSkin(dto: SkinDto, params: SkinLoaderParams = DEFAULT_SKIN_LOADER_PARAMS): Promise<SkinLoadResult> {
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
    images.set(slot, { bitmap: decoded.bitmap, width: f.width, height: decoded.height, scale: f.scale });
  }

  const bitmaps = (await Promise.all(decodes.values())).flatMap((d) => (d === null ? [] : [d.bitmap]));
  let disposed = false;
  return {
    skin: {
      version: dto.version ?? params.defaultVersion,
      columnWidth: orDefault(config.columnWidth, params.defaultColumnWidth),
      columnSpacing: orDefault(config.columnSpacing, params.defaultColumnSpacing),
      columnLineWidth: orDefault(config.columnLineWidth, params.defaultColumnLineWidth),
      hitPosition: config.hitPosition ?? params.defaultHitPosition,
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
    },
    failedSlots,
    dispose: () => {
      if (disposed) {
        return;
      }
      disposed = true;
      for (const bitmap of bitmaps) {
        bitmap.close();
      }
    },
  };
}
