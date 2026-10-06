import { describe, expect, it } from "vitest";
import { COLUMN_COLORS, columnColor } from "./colors";

describe("columnColor", () => {
  it("uses the 7K outer/inner pattern with a distinct centre column", () => {
    const { outer, inner, centre } = COLUMN_COLORS;
    expect([0, 1, 2, 3, 4, 5, 6].map((c) => columnColor(7, c))).toEqual([
      outer,
      inner,
      outer,
      centre,
      outer,
      inner,
      outer,
    ]);
    expect(new Set([outer, inner, centre]).size).toBe(3);
  });

  it("mirrors the pattern on even keymodes, which have no centre", () => {
    const { outer, inner } = COLUMN_COLORS;
    expect([0, 1, 2, 3].map((c) => columnColor(4, c))).toEqual([outer, inner, inner, outer]);
  });
});
