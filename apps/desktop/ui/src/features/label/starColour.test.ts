import { describe, expect, it } from "vitest";
import { difficultyColour, starTextColour } from "./starColour";

// osu-web's getDiffColour: d3 scaleLinear over this domain, clamped, interpolated in RGB with gamma 2.2.
const STOPS: readonly [number, string][] = [
  [0.1, "#4290FB"],
  [1.25, "#4FC0FF"],
  [2, "#4FFFD5"],
  [2.5, "#7CFF4F"],
  [3.3, "#F6F05C"],
  [4.2, "#FF8068"],
  [4.9, "#FF4E6F"],
  [5.8, "#C645B8"],
  [6.7, "#6563DE"],
  [7.7, "#18158E"],
];

describe("difficultyColour", () => {
  it.each(STOPS)("gives osu!'s exact colour at %f stars", (stars, colour) => {
    expect(difficultyColour(stars)).toBe(colour);
  });

  it("interpolates between stops in gamma 2.2 RGB, as d3's interpolateRgb.gamma(2.2)", () => {
    expect(difficultyColour(2.25)).toBe("#68FFA3");
    expect(difficultyColour(6)).toBe("#B64DC1");
  });

  it("is grey below 0.1 stars and black from 9 stars, as osu-web", () => {
    expect(difficultyColour(0)).toBe("#AAAAAA");
    expect(difficultyColour(0.05)).toBe("#AAAAAA");
    expect(difficultyColour(9)).toBe("#000000");
    expect(difficultyColour(12.4)).toBe("#000000");
  });

  it("reaches black at the last stop", () => {
    expect(difficultyColour(8.99)).toMatch(/^#0[0-9A-F]{5}$/);
  });
});

describe("starTextColour", () => {
  it("is dark below 6.5 stars and osu!'s gold from 6.5", () => {
    expect(starTextColour(1)).toBe("rgba(0, 0, 0, 0.75)");
    expect(starTextColour(6.49)).toBe("rgba(0, 0, 0, 0.75)");
    expect(starTextColour(6.5)).toBe("#FFD966");
    expect(starTextColour(10)).toBe("#FFD966");
  });
});
