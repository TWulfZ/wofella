// Test double for the subset of CanvasRenderingContext2D the playfield uses; jsdom has no canvas.

export interface FillOp {
  x: number;
  y: number;
  w: number;
  h: number;
  fill: string;
  alpha: number;
}

export interface RecordingContext {
  fillStyle: string;
  globalAlpha: number;
  fillRect(x: number, y: number, w: number, h: number): void;
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void;
}

export function recordingContext(): { ctx: RecordingContext; ops: FillOp[]; transforms: number[][] } {
  const ops: FillOp[] = [];
  const transforms: number[][] = [];
  const ctx: RecordingContext = {
    fillStyle: "#000000",
    globalAlpha: 1,
    fillRect(x, y, w, h) {
      ops.push({ x, y, w, h, fill: ctx.fillStyle, alpha: ctx.globalAlpha });
    },
    setTransform(a, b, c, d, e, f) {
      transforms.push([a, b, c, d, e, f]);
    },
  };
  return { ctx, ops, transforms };
}
