import { afterEach, describe, expect, it, vi } from "vitest";
import { PATTERN_GRID_PREFS, readAlwaysCollapsed, writeAlwaysCollapsed } from "./patternGridPrefs";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("pattern grid prefs", () => {
  it("defaults to not collapsed", () => {
    expect(readAlwaysCollapsed()).toBe(false);
  });

  it("round-trips the always-collapsed choice", () => {
    writeAlwaysCollapsed(true);
    expect(readAlwaysCollapsed()).toBe(true);
    writeAlwaysCollapsed(false);
    expect(readAlwaysCollapsed()).toBe(false);
  });

  it("reads anything but the stored true as not collapsed", () => {
    localStorage.setItem(PATTERN_GRID_PREFS.alwaysCollapsedKey, "yes");
    expect(readAlwaysCollapsed()).toBe(false);
  });

  it("reads the default and drops writes when storage throws", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    });
    expect(readAlwaysCollapsed()).toBe(false);
    expect(() => {
      writeAlwaysCollapsed(true);
    }).not.toThrow();
  });
});
