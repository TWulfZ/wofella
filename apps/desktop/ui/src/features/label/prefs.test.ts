import { afterEach, describe, expect, it, vi } from "vitest";
import { LABEL_PREFS, readOffsetMs, readScroll, writeOffsetMs, writeScroll } from "./prefs";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("audio offset preference", () => {
  it("defaults to 0 and round-trips a stored value", () => {
    expect(readOffsetMs()).toBe(0);
    writeOffsetMs(-35);
    expect(readOffsetMs()).toBe(-35);
  });

  it("clamps to the slider range and ignores garbage", () => {
    localStorage.setItem(LABEL_PREFS.offsetKey, "999");
    expect(readOffsetMs()).toBe(LABEL_PREFS.maxOffsetMs);
    localStorage.setItem(LABEL_PREFS.offsetKey, "-999");
    expect(readOffsetMs()).toBe(LABEL_PREFS.minOffsetMs);
    localStorage.setItem(LABEL_PREFS.offsetKey, "soon");
    expect(readOffsetMs()).toBe(0);
  });
});

describe("scroll preference", () => {
  it("defaults to fit and round-trips a fixed speed", () => {
    expect(readScroll()).toBe("fit");
    writeScroll(1.25);
    expect(readScroll()).toBe(1.25);
    writeScroll("fit");
    expect(readScroll()).toBe("fit");
  });

  it("clamps a stored speed and falls back to fit on garbage", () => {
    localStorage.setItem(LABEL_PREFS.scrollKey, "50");
    expect(readScroll()).toBe(LABEL_PREFS.maxPxPerMs);
    localStorage.setItem(LABEL_PREFS.scrollKey, "-1");
    expect(readScroll()).toBe(LABEL_PREFS.minPxPerMs);
    localStorage.setItem(LABEL_PREFS.scrollKey, "fast");
    expect(readScroll()).toBe("fit");
  });
});

describe("unavailable storage", () => {
  it("reads defaults and drops writes instead of throwing", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    });
    expect(readOffsetMs()).toBe(0);
    expect(readScroll()).toBe("fit");
    expect(() => {
      writeOffsetMs(10);
      writeScroll(1);
    }).not.toThrow();
  });
});
