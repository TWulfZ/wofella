import type { ColumnDto, HandDto } from "@/ipc/bindings";

const KEY_W = 10;
const GAP = 2;
const KEY_H = 22;
const PAD = 2;

const HAND_FILL: Record<HandDto, string> = {
  left: "fill-osu-pink",
  right: "fill-osu-blue",
  both: "fill-osu-purple",
};

/** A 7K-style key strip: hand colours, a ring on the thumb key and the hand split where the playfield draws it. */
export function LayoutDiagram({ columns, className }: { columns: ColumnDto[]; className?: string }) {
  const width = columns.length * KEY_W + (columns.length - 1) * GAP + PAD * 2;
  const height = KEY_H + PAD * 2;
  const keyX = (i: number) => PAD + i * (KEY_W + GAP);
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      viewBox={`0 0 ${width} ${height}`}
      className={className}
      preserveAspectRatio="xMidYMid meet"
    >
      {columns.map((column, i) => (
        <rect
          key={`key-${String(i)}`}
          data-column={i}
          data-hand={column.hand}
          x={keyX(i)}
          y={PAD}
          width={KEY_W}
          height={KEY_H}
          rx={2}
          className={HAND_FILL[column.hand]}
        />
      ))}
      {columns.map((column, i) =>
        column.finger === "thumb" ? (
          <circle
            key={`thumb-${String(i)}`}
            data-thumb={i}
            cx={keyX(i) + KEY_W / 2}
            cy={PAD + KEY_H - 5}
            r={2.6}
            className="fill-background stroke-foreground"
            strokeWidth={1}
          />
        ) : null,
      )}
      {columns.map((column, i) => {
        const prev = columns[i - 1];
        if (prev === undefined || prev.hand === column.hand) {
          return null;
        }
        const x = keyX(i) - GAP / 2;
        return (
          <line
            key={`split-${String(i)}`}
            data-split={i}
            x1={x}
            x2={x}
            y1={0}
            y2={height}
            className="stroke-foreground"
            strokeWidth={1}
            strokeDasharray="2 1.5"
          />
        );
      })}
    </svg>
  );
}
