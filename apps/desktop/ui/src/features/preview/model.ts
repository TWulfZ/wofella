// Presentation geometry only: every rating comes from the DTO, the UI derives no domain value (§8).
import type { AliasRowDto, MergeModeDto, ProfileEntryDto, SkillPreviewDto, TrendPointDto } from "@/ipc/bindings";

export const PREVIEW_PARAMS = {
  /** Radar rings every 5 rating points, a scale players already read Etterna ratings on. */
  radarRingCenti: 500,
} as const;

/** Skillsets the service flags as not measured for this keymode (ADR 0024: 7K Technical sits near 0.18). */
const UNMEASURED_BY_WARNING: Readonly<Record<string, readonly string[]>> = {
  k7_tech_not_measured: ["technical"],
};

export function unmeasuredSkillsets(warnings: readonly string[]): ReadonlySet<string> {
  return new Set(warnings.flatMap((code) => UNMEASURED_BY_WARNING[code] ?? []));
}

export interface RadarScale {
  lo: number;
  hi: number;
  rings: number[];
}

// The centre is one ring below the weakest skillset rather than 0: ratings cluster within a few points, and from 0 the
// shape would read as a near-regular polygon. The ring labels state the truncation.
export function radarScale(valuesCenti: readonly number[], ring: number = PREVIEW_PARAMS.radarRingCenti): RadarScale {
  const max = Math.max(0, ...valuesCenti);
  const min = valuesCenti.length === 0 ? 0 : Math.min(...valuesCenti);
  const hi = Math.max(ring, Math.ceil(max / ring) * ring);
  const lo = Math.min(hi - ring, Math.max(0, Math.floor(min / ring) * ring - ring));
  const rings: number[] = [];
  for (let v = lo + ring; v <= hi; v += ring) {
    rings.push(v);
  }
  return { lo, hi, rings };
}

/** Clockwise from the top, so any axis count keeps its first axis upright. */
export function axisAngle(index: number, count: number): number {
  return -Math.PI / 2 + (2 * Math.PI * index) / count;
}

function compareCodePoints(a: string, b: string): number {
  const ca = Array.from(a, (c) => c.codePointAt(0) ?? 0);
  const cb = Array.from(b, (c) => c.codePointAt(0) ?? 0);
  for (let i = 0; i < Math.min(ca.length, cb.length); i++) {
    const d = (ca[i] ?? 0) - (cb[i] ?? 0);
    if (d !== 0) {
      return d;
    }
  }
  return ca.length - cb.length;
}

/**
 * Alias names behind each preview, in the previews' order. The entry lists its scopes under the profile's own merge
 * mode, so a `?merge=separate` override yields hashes it does not know; those follow `scope::resolve`, which emits one
 * scope per alias ordered by (raw name bytes, alias id).
 */
export function scopeAliasNames(
  previews: readonly SkillPreviewDto[],
  entry: ProfileEntryDto,
  aliases: readonly AliasRowDto[],
  mergeMode: MergeModeDto | undefined,
): (string[] | undefined)[] {
  const byId = new Map(aliases.map((a) => [a.aliasId, a]));
  const known = new Map(entry.scopes.map((s) => [s.scopeHash, s.aliasIds]));
  const ordered = entry.aliasIds
    .map((id) => byId.get(id))
    .filter((a): a is AliasRowDto => a !== undefined)
    .sort((a, b) => compareCodePoints(a.rawName, b.rawName) || a.aliasId - b.aliasId);
  return previews.map((preview, index) => {
    const ids = known.get(preview.scopeHash);
    if (ids !== undefined) {
      const names = ids.map((id) => byId.get(id)?.rawName);
      return names.every((n) => n !== undefined) ? names : undefined;
    }
    if (mergeMode === "separate" && previews.length === ordered.length) {
      const alias = ordered[index];
      return alias === undefined ? undefined : [alias.rawName];
    }
    return undefined;
  });
}

export interface TrendPoint extends TrendPointDto {
  x: number;
  y: number;
}

export interface TrendGeometry {
  lo: number;
  hi: number;
  points: TrendPoint[];
}

export function trendGeometry(
  trend: readonly TrendPointDto[],
  { width, height }: { width: number; height: number },
): TrendGeometry {
  const values = trend.map((p) => p.overallCenti);
  const lo = values.length === 0 ? 0 : Math.min(...values);
  const hi = values.length === 0 ? 0 : Math.max(...values);
  const span = hi - lo;
  const step = trend.length > 1 ? width / (trend.length - 1) : 0;
  return {
    lo,
    hi,
    points: trend.map((p, i) => ({
      ...p,
      x: trend.length > 1 ? i * step : width / 2,
      y: span === 0 ? height / 2 : height - ((p.overallCenti - lo) / span) * height,
    })),
  };
}
