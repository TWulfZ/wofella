import { cn } from "@/shared/lib/utils";

/**
 * Glyphs are tiny mania playfields: 4 columns of 6 units in a 24-unit box, time running upwards like the
 * playfield (heads below tails). Accent shapes paint with `--axis-icon-accent`, so a RICE or LN card can tint
 * the defining notes by setting that variable to one of its theme tokens; the rest stay a dimmed currentColor.
 */
const ACCENT = "var(--axis-icon-accent, currentColor)";
const COLUMN = 6;
const NOTE_H = 3;
const TAIL_H = 1.5;
const RADIUS = 1;
// Dimmed rather than a second token so the glyph keeps working in any text colour the card picks.
const MUTED_OPACITY = 0.45;

type Shape =
  | { kind: "note"; col: number; y: number; accent?: boolean }
  | { kind: "ln"; col: number; head: number; tail: number; accent?: boolean };

const note = (col: number, y: number, accent = false): Shape => ({ kind: "note", col, y, accent });
const ln = (col: number, head: number, tail: number, accent = false): Shape => ({
  kind: "ln",
  col,
  head,
  tail,
  accent,
});

const GLYPHS = {
  "7k.regular.jack": [1, 2].flatMap((col) => [20.5, 14.5, 8.5, 2.5].map((y) => note(col, y, true))),
  "7k.regular.tech": [
    note(0, 19.5, true),
    note(1, 20.5),
    note(3, 17),
    note(2, 14.5, true),
    note(3, 13),
    note(0, 11),
    note(1, 8, true),
    note(3, 6.5),
    note(0, 3.5),
    note(2, 1.5, true),
  ],
  "7k.regular.speed": [0, 1, 2, 3, 0, 1, 2, 3].map((col, i) => note(col, 20.5 - i * 2.75, i < 4)),
  "7k.regular.stream": [
    note(1, 20.5, true),
    note(3, 16.25, true),
    note(0, 12),
    note(2, 12),
    note(3, 7.75, true),
    note(1, 3.5, true),
  ],
  "7k.ln.general": [ln(0, 20.5, 1.5, true), ln(2, 20.5, 1.5, true), ln(3, 15, 5.5)],
  "7k.ln.tech": [
    ln(1, 20.5, 8, true),
    note(1, 2, true),
    note(3, 19),
    note(0, 16),
    note(2, 12.5),
    note(3, 6.5),
    note(2, 2.5),
  ],
  "7k.ln.inverse": [ln(1, 20.5, 12, true), ln(1, 8, 0.5, true), ln(3, 16.5, 7.5), ln(3, 4, 0.5)],
  "7k.ln.release": [14, 10, 6, 2].map((tail, col) => ln(col, 20.5, tail, true)),
} satisfies Record<string, Shape[]>;

type AxisIconId = keyof typeof GLYPHS;

export const AXIS_ICON_IDS = Object.keys(GLYPHS) as readonly AxisIconId[];

function isAxisIconId(axis: string): axis is AxisIconId {
  return Object.hasOwn(GLYPHS, axis);
}

function paint(accent: boolean | undefined) {
  return accent === true ? { fill: ACCENT } : { fill: "currentColor", fillOpacity: MUTED_OPACITY };
}

function ShapeView({ shape }: { shape: Shape }) {
  const x = shape.col * COLUMN;
  const colour = paint(shape.accent);
  if (shape.kind === "note") {
    return <rect data-shape="note" x={x + 0.5} y={shape.y} width={COLUMN - 1} height={NOTE_H} rx={RADIUS} {...colour} />;
  }
  return (
    <g>
      <rect
        data-shape="ln-body"
        x={x + 2}
        y={shape.tail}
        width={2}
        height={shape.head - shape.tail + NOTE_H / 2}
        rx={RADIUS}
        {...colour}
      />
      <rect data-shape="ln-tail" x={x + 1} y={shape.tail} width={COLUMN - 2} height={TAIL_H} rx={TAIL_H / 2} {...colour} />
      <rect
        data-shape="note"
        x={x + 0.5}
        y={shape.head}
        width={COLUMN - 1}
        height={NOTE_H}
        rx={RADIUS}
        {...colour}
      />
    </g>
  );
}

export function AxisIcon({ axis, className }: { axis: string; className?: string }) {
  const known = isAxisIconId(axis);
  return (
    <svg
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
      data-axis-icon={known ? axis : "fallback"}
      className={cn("size-5 shrink-0", className)}
    >
      {known ? (
        GLYPHS[axis].map((shape, i) => <ShapeView key={i} shape={shape} />)
      ) : (
        // An empty note outline: "some pattern" without claiming an axis the taxonomy does not define.
        <rect
          data-shape="note"
          x={3.5}
          y={10.5}
          width={17}
          height={NOTE_H}
          rx={RADIUS}
          fill="none"
          stroke="currentColor"
          strokeWidth={1}
          strokeDasharray="2 1.5"
        />
      )}
    </svg>
  );
}
