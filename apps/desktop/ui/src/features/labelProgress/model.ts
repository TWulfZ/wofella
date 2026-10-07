// Display shaping only: every count comes from the progress and session DTOs (§8).
import { groupByAxis, groupByFamily } from "@/features/label";
import type { CountDto, DayCountDto, LabelProgressDto, PatternDefDto, SessionPlayDto } from "@/ipc/bindings";

export interface SessionMapCounts {
  labelled: number;
  pending: number;
}

/** Maps, not plays: the answer is about the chart (ADR 0020), so a map played twice is one to label. */
export function sessionMapCounts(plays: readonly SessionPlayDto[]): SessionMapCounts {
  const labelled = new Set<string>();
  const pending = new Set<string>();
  for (const play of plays) {
    (play.label === null ? pending : labelled).add(play.md5);
  }
  return { labelled: labelled.size, pending: pending.size };
}

/** The newest play of each map without an answer; `plays` arrive newest first. */
export function pendingMaps(plays: readonly SessionPlayDto[]): SessionPlayDto[] {
  const seen = new Set<string>();
  return plays.filter((play) => {
    if (play.label !== null || seen.has(play.md5)) {
      return false;
    }
    seen.add(play.md5);
    return true;
  });
}

export function newestPlayOf(plays: readonly SessionPlayDto[], md5: string): SessionPlayDto | null {
  return plays.find((play) => play.md5 === md5) ?? null;
}

export interface SessionMap {
  /** The map's newest play: its id goes with the answer as provenance. */
  newest: SessionPlayDto;
  playCount: number;
}

export interface SessionMaps {
  pending: SessionMap[];
  labelled: SessionMap[];
}

/** One entry per map, newest play first, split by whether the map has an answer; `plays` arrive newest first. */
export function sessionMaps(plays: readonly SessionPlayDto[]): SessionMaps {
  const byMd5 = new Map<string, SessionMap>();
  for (const play of plays) {
    const entry = byMd5.get(play.md5);
    if (entry === undefined) {
      byMd5.set(play.md5, { newest: play, playCount: 1 });
    } else {
      entry.playCount += 1;
    }
  }
  const maps = [...byMd5.values()];
  return {
    pending: maps.filter((m) => m.newest.label === null),
    labelled: maps.filter((m) => m.newest.label !== null),
  };
}

export interface DayTotals {
  gold: number;
  session: number;
}

/** The series ends on the service's today at the request's offset. */
export function todayCounts(perDay: readonly DayCountDto[]): DayTotals {
  const today = perDay.at(-1);
  return { gold: today?.gold ?? 0, session: today?.session ?? 0 };
}

export interface ActivityTotals extends DayTotals {
  activeDays: number;
  /** The busiest day's gold plus session count, the scale of the bars. */
  peak: number;
}

export function activityTotals(perDay: readonly DayCountDto[]): ActivityTotals {
  let gold = 0;
  let session = 0;
  let activeDays = 0;
  let peak = 0;
  for (const day of perDay) {
    const total = day.gold + day.session;
    gold += day.gold;
    session += day.session;
    activeDays += total > 0 ? 1 : 0;
    peak = Math.max(peak, total);
  }
  return { gold, session, activeDays, peak };
}

export interface PatternBar {
  id: string;
  count: number;
}

export interface AxisBar {
  axis: string;
  count: number;
  patterns: PatternBar[];
}

export interface FamilyBars {
  family: string;
  axes: AxisBar[];
}

/** Every axis and pattern of the taxonomy, in its order, so the gaps still to label show as empty bars. */
export function axisBars(
  taxonomy: readonly PatternDefDto[],
  perAxis: readonly CountDto[],
  perPattern: readonly CountDto[],
): FamilyBars[] {
  const axisCount = new Map(perAxis.map(({ key, count }) => [key, count]));
  const patternCount = new Map(perPattern.map(({ key, count }) => [key, count]));
  return groupByFamily(groupByAxis(taxonomy)).map(({ family, axes }) => ({
    family,
    axes: axes.map(({ axis, patterns }) => ({
      axis,
      count: axisCount.get(axis) ?? 0,
      patterns: patterns.map(({ id }) => ({ id, count: patternCount.get(id) ?? 0 })),
    })),
  }));
}

export interface GoldOrigins {
  /** A `sampled` or `random` pick of the offered window (ADR 0021): what evaluation uses by default. */
  blind: number;
  /** Picked by the labeller or moved off the offered window. */
  chosen: number;
  /** Stored before the origin was recorded. */
  unknown: number;
}

export function goldOrigins(progress: Pick<LabelProgressDto, "goldTotal" | "goldBlind" | "perSelection">): GoldOrigins {
  const unknown = progress.perSelection.reduce((sum, row) => sum + (row.selection === null ? row.count : 0), 0);
  return { blind: progress.goldBlind, chosen: progress.goldTotal - progress.goldBlind - unknown, unknown };
}
