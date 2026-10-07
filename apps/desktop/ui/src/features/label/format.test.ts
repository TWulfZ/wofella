import { describe, expect, it } from "vitest";
import { formatClock, formatMinSec } from "./format";

describe("formatClock", () => {
  it("prints mm:ss.mmm like the CLI", () => {
    expect(formatClock(0)).toBe("00:00.000");
    expect(formatClock(1000)).toBe("00:01.000");
    expect(formatClock(61_234)).toBe("01:01.234");
    expect(formatClock(-1500)).toBe("-00:01.500");
  });
});

describe("formatMinSec", () => {
  it("prints mm:ss, truncating the milliseconds", () => {
    expect(formatMinSec(0)).toBe("00:00");
    expect(formatMinSec(83_999)).toBe("01:23");
    expect(formatMinSec(3_600_000)).toBe("60:00");
  });
});
