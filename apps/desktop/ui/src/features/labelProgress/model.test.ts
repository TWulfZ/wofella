import { describe, expect, it } from "vitest";
import type { DayCountDto, PatternDefDto, SessionPlayDto } from "@/ipc/bindings";
import { activityTotals, axisBars, goldOrigins, newestPlayOf, pendingMaps, sessionMapCounts, todayCounts } from "./model";

function play(md5: string, playedAt: string, label: SessionPlayDto["label"] = null): SessionPlayDto {
  return {
    playId: `${md5.slice(0, 4)}${playedAt}`,
    md5,
    playedAt,
    title: `Song ${md5.slice(0, 1)}`,
    artist: "Artist",
    version: "Insane",
    creator: "Mapper",
    stars: 4.2,
    keymode: 7,
    setId: null,
    label,
    goldWindows: 0,
  };
}

const A = "a".repeat(32);
const B = "b".repeat(32);
const C = "c".repeat(32);
const SAVED = { eventId: "01E", pattern: "7k.regular.jack.minijack", at: "2026-10-07T10:00:00.000Z" };

describe("sessionMapCounts", () => {
  it("counts maps, not plays: a map played twice is one pending or labelled map", () => {
    const plays = [play(A, "3"), play(B, "2", SAVED), play(A, "1"), play(C, "0")];
    expect(sessionMapCounts(plays)).toEqual({ labelled: 1, pending: 2 });
  });

  it("is zero for an empty session", () => {
    expect(sessionMapCounts([])).toEqual({ labelled: 0, pending: 0 });
  });
});

describe("pendingMaps", () => {
  it("keeps the newest play of each unlabelled map, in the list's newest-first order", () => {
    const plays = [play(A, "3"), play(B, "2", SAVED), play(C, "1"), play(A, "0")];
    expect(pendingMaps(plays).map((p) => p.playId)).toEqual([play(A, "3").playId, play(C, "1").playId]);
  });
});

function day(d: string, gold: number, session: number): DayCountDto {
  return { day: d, gold, session };
}

describe("todayCounts", () => {
  it("reads the last day of the series, which the service cuts at the local offset", () => {
    expect(todayCounts([day("2026-10-06", 4, 1), day("2026-10-07", 2, 3)])).toEqual({ gold: 2, session: 3 });
  });

  it("is zero without days", () => {
    expect(todayCounts([])).toEqual({ gold: 0, session: 0 });
  });
});

describe("activityTotals", () => {
  it("sums the window and counts the days with any labelling", () => {
    expect(activityTotals([day("1", 0, 0), day("2", 3, 0), day("3", 0, 2), day("4", 1, 1)])).toEqual({
      gold: 4,
      session: 3,
      activeDays: 3,
      peak: 3,
    });
  });
});

const TAXONOMY: PatternDefDto[] = [
  { id: "7k.regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "" },
  { id: "7k.regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "" },
  { id: "7k.regular.speed.trill", axis: "7k.regular.speed", key: "tr", description: "" },
  { id: "7k.ln.release.shield", axis: "7k.ln.release", key: "sh", description: "" },
];

describe("axisBars", () => {
  it("groups the taxonomy into RICE and LN families with each axis and pattern count, zeros included", () => {
    const bars = axisBars(
      TAXONOMY,
      [
        { key: "7k.ln.release", count: 2 },
        { key: "7k.regular.jack", count: 5 },
      ],
      [
        { key: "7k.ln.release.shield", count: 2 },
        { key: "7k.regular.jack.longjack", count: 1 },
        { key: "7k.regular.jack.minijack", count: 4 },
      ],
    );
    expect(bars).toEqual([
      {
        family: "regular",
        axes: [
          {
            axis: "7k.regular.jack",
            count: 5,
            patterns: [
              { id: "7k.regular.jack.minijack", count: 4 },
              { id: "7k.regular.jack.longjack", count: 1 },
            ],
          },
          { axis: "7k.regular.speed", count: 0, patterns: [{ id: "7k.regular.speed.trill", count: 0 }] },
        ],
      },
      {
        family: "ln",
        axes: [{ axis: "7k.ln.release", count: 2, patterns: [{ id: "7k.ln.release.shield", count: 2 }] }],
      },
    ]);
  });
});

describe("newestPlayOf", () => {
  it("finds the map's newest play in the newest-first list, or null when the session never played it", () => {
    const plays = [play(A, "3"), play(B, "2", SAVED), play(A, "1")];
    expect(newestPlayOf(plays, A)?.playId).toBe(play(A, "3").playId);
    expect(newestPlayOf(plays, C)).toBeNull();
  });
});

describe("goldOrigins", () => {
  it("splits the gold total into blind, chosen and labels stored before the origin was recorded", () => {
    expect(
      goldOrigins({
        goldTotal: 20,
        goldBlind: 9,
        perSelection: [
          { selection: null, count: 6 },
          { selection: { pick: "random", window: "sampled" }, count: 4 },
          { selection: { pick: "sampled", window: "sampled" }, count: 5 },
          { selection: { pick: "sampled", window: "moved" }, count: 2 },
          { selection: { pick: "session", window: "sampled" }, count: 3 },
        ],
      }),
    ).toEqual({ blind: 9, chosen: 5, unknown: 6 });
  });

  it("is all zero without gold labels", () => {
    expect(goldOrigins({ goldTotal: 0, goldBlind: 0, perSelection: [] })).toEqual({ blind: 0, chosen: 0, unknown: 0 });
  });
});
