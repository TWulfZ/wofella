import { describe, expect, it } from "vitest";
import type { ProfileEntryDto } from "@/ipc/bindings";
import { ALIASES, ready4k, ready7k, SELF_4K } from "./fixtures";
import { axisAngle, radarScale, scopeAliasNames, trendGeometry, unmeasuredSkillsets } from "./model";

describe("radarScale", () => {
  it("brackets the measured values on whole 5-point rings, with one ring of headroom below", () => {
    expect(radarScale([1990, 2601])).toEqual({ lo: 1000, hi: 3000, rings: [1500, 2000, 2500, 3000] });
  });

  it("never goes below zero and keeps one ring for an empty or all-zero chart", () => {
    expect(radarScale([])).toEqual({ lo: 0, hi: 500, rings: [500] });
    expect(radarScale([0, 120])).toEqual({ lo: 0, hi: 500, rings: [500] });
  });
});

describe("axisAngle", () => {
  it("starts at the top and goes clockwise for any axis count", () => {
    expect(axisAngle(0, 7)).toBeCloseTo(-Math.PI / 2);
    expect(axisAngle(1, 4)).toBeCloseTo(0);
    expect(axisAngle(3, 6)).toBeCloseTo(Math.PI / 2);
  });
});

describe("unmeasuredSkillsets", () => {
  it("marks 7K technical only when the service says it is not measured", () => {
    expect([...unmeasuredSkillsets(ready7k().warnings)]).toEqual(["technical"]);
    expect(unmeasuredSkillsets(ready4k().warnings).size).toBe(0);
  });
});

describe("scopeAliasNames", () => {
  const twoNames: ProfileEntryDto = {
    ...SELF_4K,
    mergeMode: "separate",
    scopes: [
      { scopeHash: "1".repeat(64), aliasIds: [2], keymode: 4 },
      { scopeHash: "2".repeat(64), aliasIds: [1], keymode: 4 },
    ],
  };

  it("names a scope by its aliases when the entry lists it", () => {
    const previews = [ready4k({ scopeHash: "2".repeat(64) })];
    expect(scopeAliasNames(previews, twoNames, ALIASES.aliases, "separate")).toEqual([["TWulfZ"]]);
  });

  it("falls back to the service's name order when the URL overrides the profile's merge mode", () => {
    const previews = [ready4k({ scopeHash: "8".repeat(64) }), ready4k({ scopeHash: "9".repeat(64) })];
    expect(scopeAliasNames(previews, SELF_4K, ALIASES.aliases, "separate")).toEqual([
      ["TWulfZ"],
      ["TWulfZasdasdasd d jSS||"],
    ]);
  });

  it("gives no name to a merged scope it cannot find", () => {
    const previews = [ready4k({ scopeHash: "8".repeat(64) })];
    expect(scopeAliasNames(previews, SELF_4K, ALIASES.aliases, "merged")).toEqual([undefined]);
  });
});

describe("trendGeometry", () => {
  it("spreads months over the width and scales Overall between its bounds", () => {
    const g = trendGeometry(ready4k().trend, { width: 200, height: 100 });
    expect(g.lo).toBe(2210);
    expect(g.hi).toBe(2436);
    expect(g.points.map((p) => p.x)).toEqual([0, 100, 200]);
    expect(g.points[0]?.y).toBeCloseTo(100);
    expect(g.points[2]?.y).toBeCloseTo(0);
  });

  it("centres a single month and a flat series", () => {
    const g = trendGeometry([{ month: "2026-09", overallCenti: 2000 }], { width: 200, height: 100 });
    expect(g.points).toEqual([{ x: 100, y: 50, month: "2026-09", overallCenti: 2000 }]);
  });
});
