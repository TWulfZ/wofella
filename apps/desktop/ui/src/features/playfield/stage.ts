export type ScrollMode = { kind: "osu"; speed: number } | { kind: "pxPerMs"; value: number };

/** osu! stable lays the mania stage out in a virtual space 480 px tall; skin.ini values (HitPosition, ColumnWidth) live there. */
export interface StageParams {
  virtualHeightPx: number;
  /** Stable's velocity per speed step, `21n/600` virtual px/ms (ppy/osu PR #13901). */
  osuPxPerMsPer480: number;
  minOsuSpeed: number;
  maxOsuSpeed: number;
  osuSpeedStep: number;
  /** skin.ini default; lazer's LegacyManiaSkinDecoder clamps HitPosition to [240, 480]. */
  defaultHitPosition: number;
  minHitPosition: number;
  maxHitPosition: number;
  /** Used until a skin provides its own ColumnWidth. */
  defaultColumnWidth: number;
  minZoom: number;
  maxZoom: number;
  defaultZoom: number;
}

export const DEFAULT_STAGE_PARAMS: StageParams = {
  virtualHeightPx: 480,
  osuPxPerMsPer480: 0.035,
  minOsuSpeed: 1,
  maxOsuSpeed: 40,
  osuSpeedStep: 1,
  defaultHitPosition: 402,
  minHitPosition: 240,
  maxHitPosition: 480,
  defaultColumnWidth: 30,
  minZoom: 0.75,
  maxZoom: 2,
  defaultZoom: 1,
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

export function clampOsuSpeed(speed: number, params: StageParams = DEFAULT_STAGE_PARAMS): number {
  if (!Number.isFinite(speed)) {
    return params.minOsuSpeed;
  }
  const stepped = Math.round(speed / params.osuSpeedStep) * params.osuSpeedStep;
  return clamp(stepped, params.minOsuSpeed, params.maxOsuSpeed);
}

export function clampZoom(zoom: number, params: StageParams = DEFAULT_STAGE_PARAMS): number {
  return Number.isFinite(zoom) ? clamp(zoom, params.minZoom, params.maxZoom) : params.defaultZoom;
}

export function osuPxPerMs(speed: number, heightPx: number, params: StageParams = DEFAULT_STAGE_PARAMS): number {
  return params.osuPxPerMsPer480 * speed * (heightPx / params.virtualHeightPx);
}

/**
 * Map-time scroll velocity. The real-time velocity stays fixed under a rate mod, so in map time it is divided by the
 * rate (ppy/osu PR #8887 scales lazer's time range by tempo for the same reason).
 */
export function scrollPxPerMs(
  mode: ScrollMode,
  heightPx: number,
  rate = 1,
  params: StageParams = DEFAULT_STAGE_PARAMS,
): number {
  const realTime = mode.kind === "osu" ? osuPxPerMs(clampOsuSpeed(mode.speed, params), heightPx, params) : mode.value;
  return realTime / rate;
}

/** Time a note takes from the top edge to the judgement line. */
export function visibleMs(pxPerMs: number, judgeY: number): number {
  return judgeY / pxPerMs;
}

export function judgeYFromHitPosition(
  hitPosition: number,
  heightPx: number,
  params: StageParams = DEFAULT_STAGE_PARAMS,
): number {
  return (clamp(hitPosition, params.minHitPosition, params.maxHitPosition) * heightPx) / params.virtualHeightPx;
}

export function uniformColumnWidths(columns: number, params: StageParams = DEFAULT_STAGE_PARAMS): number[] {
  return Array.from({ length: columns }, () => params.defaultColumnWidth);
}

export function stageWidthPx(
  columnWidths: readonly number[],
  heightPx: number,
  zoom: number,
  params: StageParams = DEFAULT_STAGE_PARAMS,
): number {
  const width480 = columnWidths.reduce((sum, w) => sum + w, 0);
  return (width480 * heightPx * clampZoom(zoom, params)) / params.virtualHeightPx;
}
