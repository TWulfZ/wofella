import type { SVGProps } from "react";

// osu!web's beatmap stat glyphs (length, BPM, circles, sliders), redrawn for mania: notes and long notes.

type IconProps = SVGProps<SVGSVGElement>;

function Glyph({ children, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...props}
    >
      {children}
    </svg>
  );
}

export function LengthIcon(props: IconProps) {
  return (
    <Glyph {...props}>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7v5l3 2" />
    </Glyph>
  );
}

export function BpmIcon(props: IconProps) {
  return (
    <Glyph {...props}>
      <path d="M9 3h6l4 18H5z" />
      <path d="M12 16 16.5 6" />
      <path d="M7.5 16h9" />
    </Glyph>
  );
}

export function NoteCountIcon(props: IconProps) {
  return (
    <Glyph {...props}>
      <circle cx="12" cy="12" r="8" />
      <circle cx="12" cy="12" r="3" fill="currentColor" />
    </Glyph>
  );
}

export function LongNoteCountIcon(props: IconProps) {
  return (
    <Glyph {...props}>
      <rect x="8" y="3" width="8" height="18" rx="4" />
      <path d="M8 17h8" />
    </Glyph>
  );
}
