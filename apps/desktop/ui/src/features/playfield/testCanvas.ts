// Test double for the subset of CanvasRenderingContext2D the playfield uses; jsdom has no canvas.

export interface FillOp {
  x: number;
  y: number;
  w: number;
  h: number;
  fill: string;
  alpha: number;
}

export interface ImageOp {
  image: CanvasImageSource;
  sx: number;
  sy: number;
  sw: number;
  sh: number;
  dx: number;
  dy: number;
  dw: number;
  dh: number;
  alpha: number;
  /** Current transform as [a, b, c, d, e, f], with -0 read as 0. */
  transform: number[];
}

export type Op = ({ type: "fill" } & FillOp) | ({ type: "image" } & ImageOp);

export interface RecordingContext {
  fillStyle: string;
  globalAlpha: number;
  fillRect(x: number, y: number, w: number, h: number): void;
  drawImage(
    image: CanvasImageSource,
    sx: number,
    sy: number,
    sw: number,
    sh: number,
    dx: number,
    dy: number,
    dw: number,
    dh: number,
  ): void;
  save(): void;
  restore(): void;
  translate(x: number, y: number): void;
  scale(x: number, y: number): void;
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void;
}

export function recordingContext(): {
  ctx: RecordingContext;
  ops: FillOp[];
  images: ImageOp[];
  all: Op[];
  transforms: number[][];
} {
  const ops: FillOp[] = [];
  const images: ImageOp[] = [];
  const all: Op[] = [];
  const transforms: number[][] = [];
  let m = [1, 0, 0, 1, 0, 0];
  const stack: number[][] = [];
  const at = (i: number): number => m[i] ?? 0;
  const ctx: RecordingContext = {
    fillStyle: "#000000",
    globalAlpha: 1,
    fillRect(x, y, w, h) {
      const op = { x, y, w, h, fill: ctx.fillStyle, alpha: ctx.globalAlpha };
      ops.push(op);
      all.push({ type: "fill", ...op });
    },
    drawImage(image, sx, sy, sw, sh, dx, dy, dw, dh) {
      const op = { image, sx, sy, sw, sh, dx, dy, dw, dh, alpha: ctx.globalAlpha, transform: m.map((v) => v + 0) };
      images.push(op);
      all.push({ type: "image", ...op });
    },
    save() {
      stack.push([...m]);
    },
    restore() {
      m = stack.pop() ?? m;
    },
    translate(x, y) {
      m = [at(0), at(1), at(2), at(3), at(4) + at(0) * x + at(2) * y, at(5) + at(1) * x + at(3) * y];
    },
    scale(x, y) {
      m = [at(0) * x, at(1) * x, at(2) * y, at(3) * y, at(4), at(5)];
    },
    setTransform(a, b, c, d, e, f) {
      transforms.push([a, b, c, d, e, f]);
      m = [a, b, c, d, e, f];
    },
  };
  return { ctx, ops, images, all, transforms };
}
