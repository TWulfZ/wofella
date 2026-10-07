import { afterEach, describe, expect, it, vi } from "vitest";
import { PATTERN_GRID_PREFS, readOpenAxes, writeOpenAxes } from "./patternGridPrefs";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("pattern grid prefs", () => {
  it("defaults to no open axis", () => {
    expect([...readOpenAxes()]).toEqual([]);
  });

  it("round-trips the open axes", () => {
    writeOpenAxes(["7k.regular.jack", "7k.ln.release"]);
    expect([...readOpenAxes()]).toEqual(["7k.regular.jack", "7k.ln.release"]);
    writeOpenAxes([]);
    expect([...readOpenAxes()]).toEqual([]);
  });

  it.each([
    ["not JSON", "{"],
    ["not an array", '{"7k.regular.jack":true}'],
  ])("reads %s as no open axis", (_, stored) => {
    localStorage.setItem(PATTERN_GRID_PREFS.openAxesKey, stored);
    expect([...readOpenAxes()]).toEqual([]);
  });

  it("drops entries that are not axis ids", () => {
    localStorage.setItem(PATTERN_GRID_PREFS.openAxesKey, '["7k.regular.jack", 3, null]');
    expect([...readOpenAxes()]).toEqual(["7k.regular.jack"]);
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
    expect([...readOpenAxes()]).toEqual([]);
    expect(() => {
      writeOpenAxes(["7k.regular.jack"]);
    }).not.toThrow();
  });
});
