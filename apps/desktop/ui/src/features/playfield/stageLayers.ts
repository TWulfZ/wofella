import type { Draw2D } from "./draw";
import { createSurface, type Surface, type SurfaceFactory } from "./offscreen";

export interface StageBackgroundSize {
  /** CSS px. */
  width: number;
  height: number;
  dpr: number;
}

export interface StageBackgroundImage {
  image: CanvasImageSource;
  pxWidth: number;
  pxHeight: number;
}

/**
 * Only the opaque background is cached: on WSLg's software renderer every extra full-canvas blend costs more than the
 * few lines it would replace (measured on WSLg: 1.9–2.1 ms per frame with the background alone, 2.6–3.1 ms uncached).
 */
export interface StageBackground {
  /**
   * The background for `key` at `size`, painted through `paint` in CSS px only when either differs from the last call.
   * Null when no offscreen surface can be made; the caller then draws the background itself.
   */
  get(key: readonly unknown[], size: StageBackgroundSize, paint: (ctx: Draw2D) => void): StageBackgroundImage | null;
  /** Frees the surface now; the next `get` paints again. */
  release(): void;
}

export function createStageBackground(factory: SurfaceFactory = createSurface): StageBackground {
  let lastKey: readonly unknown[] | null = null;
  let built: { result: StageBackgroundImage; surface: Surface } | null = null;

  return {
    get(key, size, paint) {
      const fullKey = [...key, size.width, size.height, size.dpr];
      if (lastKey !== null && sameKey(lastKey, fullKey)) {
        return built?.result ?? null;
      }
      built?.surface.release();
      built = null;
      lastKey = fullKey;
      const pxWidth = Math.max(1, Math.round(size.width * size.dpr));
      const pxHeight = Math.max(1, Math.round(size.height * size.dpr));
      const surface = factory(pxWidth, pxHeight);
      if (surface === null) {
        return null;
      }
      surface.ctx.scale(size.dpr, size.dpr);
      paint(surface.ctx);
      built = { result: { image: surface.image, pxWidth, pxHeight }, surface };
      return built.result;
    },
    release() {
      built?.surface.release();
      built = null;
      lastKey = null;
    },
  };
}

function sameKey(a: readonly unknown[], b: readonly unknown[]): boolean {
  return a.length === b.length && a.every((v, i) => Object.is(v, b[i]));
}
