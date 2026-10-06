import { describe, expect, it } from "vitest";
import { formatClock } from "./format";

describe("formatClock", () => {
  it("prints mm:ss.mmm like the CLI", () => {
    expect(formatClock(0)).toBe("00:00.000");
    expect(formatClock(1000)).toBe("00:01.000");
    expect(formatClock(61_234)).toBe("01:01.234");
    expect(formatClock(-1500)).toBe("-00:01.500");
  });
});
