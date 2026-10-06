import type { Draw2D } from "./draw";

/** A 2D canvas off the page: painted once through `ctx`, then drawn as `image`. */
export interface Surface {
  image: CanvasImageSource;
  ctx: Draw2D;
  /** Frees the backing store now rather than at garbage collection. */
  release: () => void;
}

export type SurfaceFactory = (pxWidth: number, pxHeight: number) => Surface | null;

/**
 * Both target webviews (WebView2, WebKitGTK) have OffscreenCanvas. Elsewhere this yields null and callers keep their
 * direct path: the stage is drawn every frame and LN tails are drawn procedurally.
 */
export const createSurface: SurfaceFactory = (pxWidth, pxHeight) => {
  if (typeof OffscreenCanvas !== "function") {
    return null;
  }
  const canvas = new OffscreenCanvas(pxWidth, pxHeight);
  const ctx = canvas.getContext("2d");
  if (ctx === null) {
    return null;
  }
  return {
    image: canvas,
    ctx,
    release: () => {
      canvas.width = 0;
      canvas.height = 0;
    },
  };
};
