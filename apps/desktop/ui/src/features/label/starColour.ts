// osu!'s star-rating colours, as osu-web's getDiffColour draws them (resources/js/utils/beatmap-helper.ts). The
// literal colours are data that players recognise from the game, not theme colours.

const DOMAIN: readonly number[] = [0.1, 1.25, 2, 2.5, 3.3, 4.2, 4.9, 5.8, 6.7, 7.7, 9];
const RANGE: readonly string[] = [
  "#4290FB",
  "#4FC0FF",
  "#4FFFD5",
  "#7CFF4F",
  "#F6F05C",
  "#FF8068",
  "#FF4E6F",
  "#C645B8",
  "#6563DE",
  "#18158E",
  "#000000",
];
const BELOW_SPECTRUM = "#AAAAAA";
const ABOVE_SPECTRUM = "#000000";
// d3's interpolateRgb.gamma(2.2), which osu-web interpolates the spectrum with.
const GAMMA = 2.2;
const GOLD_TEXT_FROM_STARS = 6.5;
const GOLD_TEXT = "#FFD966";
const DARK_TEXT = "rgba(0, 0, 0, 0.75)";

type Rgb = readonly [number, number, number];

function parseHex(hex: string): Rgb {
  const n = Number.parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function toHex(rgb: Rgb): string {
  return `#${rgb.map((c) => Math.round(Math.min(255, Math.max(0, c))).toString(16).padStart(2, "0")).join("")}`.toUpperCase();
}

function gammaMix(a: number, b: number, t: number): number {
  const ga = a ** GAMMA;
  return (ga + t * (b ** GAMMA - ga)) ** (1 / GAMMA);
}

/** Background colour of a star-rating badge, `#RRGGBB`. */
export function difficultyColour(stars: number): string {
  if (stars < (DOMAIN[0] ?? 0)) {
    return BELOW_SPECTRUM;
  }
  if (stars >= (DOMAIN.at(-1) ?? Infinity)) {
    return ABOVE_SPECTRUM;
  }
  const upper = DOMAIN.findIndex((d) => d > stars);
  const lo = DOMAIN[upper - 1] ?? 0;
  const hi = DOMAIN[upper] ?? lo;
  const from = parseHex(RANGE[upper - 1] ?? ABOVE_SPECTRUM);
  const to = parseHex(RANGE[upper] ?? ABOVE_SPECTRUM);
  const t = hi === lo ? 0 : (stars - lo) / (hi - lo);
  return toHex([gammaMix(from[0], to[0], t), gammaMix(from[1], to[1], t), gammaMix(from[2], to[2], t)]);
}

/** Text colour on a badge of `difficultyColour(stars)`. */
export function starTextColour(stars: number): string {
  return stars >= GOLD_TEXT_FROM_STARS ? GOLD_TEXT : DARK_TEXT;
}
