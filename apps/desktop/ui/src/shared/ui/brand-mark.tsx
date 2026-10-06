import { useId } from "react";
import { cn } from "cn";

const WHITE = "white";
// Default 7K skin column colours, left to right; the middle column is the yellow "space" key.
const COLUMNS = [WHITE, "var(--osu-blue)", WHITE, "var(--osu-yellow)", WHITE, "var(--osu-blue)", WHITE] as const;
const NOTE_WIDTH = 3;
const NOTE_HEIGHT = 4.2;
const COLUMN_PITCH = 3.6;
const FIRST_COLUMN_X = 3.7;
const LOWEST_NOTE_Y = 21.1;
const STAIR_RISE = 2.4;

/** Original mania stair mark (one note per 7K column); deliberately not the osu! logo. */
export function BrandMark({ className }: { className?: string }) {
  const gradientId = useId();
  return (
    <svg viewBox="0 0 32 32" aria-hidden="true" className={cn("shrink-0", className)}>
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="var(--osu-pink)" />
          <stop offset="100%" stopColor="var(--osu-purple)" />
        </linearGradient>
      </defs>
      <rect width="32" height="32" rx="8" fill={`url(#${gradientId})`} />
      {COLUMNS.map((color, i) => (
        <rect
          key={i}
          x={FIRST_COLUMN_X + i * COLUMN_PITCH}
          y={LOWEST_NOTE_Y - i * STAIR_RISE}
          width={NOTE_WIDTH}
          height={NOTE_HEIGHT}
          rx="1"
          fill={color}
          fillOpacity={color === WHITE ? 0.95 : 1}
        />
      ))}
    </svg>
  );
}
