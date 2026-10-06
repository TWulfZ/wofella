import { cn } from "cn";

// Logo A1 from the design canvas; src-tauri/icons/app-icon.svg draws the same polygon on the app-icon tile.
const W_MARK =
  "27.5,82.5 4,4.1 25.2,17.5 36.1,53.8 43.4,29.6 60.6,29.6 67.9,53.8 78.8,17.5 96,17.5 76.5,82.5 80.5,95.9 59.3,82.5 52,58.3 44.7,82.5";

export function BrandMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 100 100" aria-hidden="true" className={cn("shrink-0", className)}>
      <polygon points={W_MARK} fill="var(--osu-pink)" />
    </svg>
  );
}
