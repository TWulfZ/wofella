import { useTranslation } from "react-i18next";

const RICE_AXES = ["regular_jack", "regular_tech", "regular_speed", "regular_stream"] as const;
const LN_AXES = ["ln_general", "ln_tech", "ln_inverse", "ln_release"] as const;

// Illustrative shapes only; they are never derived from the player's data (see the preview caption).
const RICE_SHAPE = [0.82, 0.58, 0.4, 0.66];
const LN_SHAPE = [0.52, 0.36, 0.62, 0.44];
const TREND_A = [0.3, 0.34, 0.33, 0.41, 0.45, 0.44, 0.52, 0.58];
const TREND_B = [0.22, 0.2, 0.26, 0.25, 0.3, 0.35, 0.34, 0.4];

const CENTER = 100;
const RADIUS = 56;
const RINGS = [1 / 3, 2 / 3, 1];
// Clockwise from the top, so the four axes sit on the diamond's tips.
const ANGLES = [-Math.PI / 2, 0, Math.PI / 2, Math.PI];

function coords(angle: number, r: number): [number, number] {
  return [CENTER + Math.cos(angle) * RADIUS * r, CENTER + Math.sin(angle) * RADIUS * r];
}

function point(angle: number, r: number): string {
  return coords(angle, r)
    .map((c) => c.toFixed(1))
    .join(",");
}

function polygon(values: readonly number[], pad = 0): string {
  return ANGLES.map((angle, i) => point(angle, Math.min(1, (values[i] ?? 0) + pad))).join(" ");
}

const LABEL_POS = [
  { x: CENTER, y: CENTER - RADIUS - 12, anchor: "middle" },
  { x: CENTER + RADIUS + 8, y: CENTER + 4, anchor: "start" },
  { x: CENTER, y: CENTER + RADIUS + 20, anchor: "middle" },
  { x: CENTER - RADIUS - 8, y: CENTER + 4, anchor: "end" },
] as const;

function Radar({
  title,
  axes,
  shape,
  color,
}: {
  title: string;
  axes: readonly string[];
  shape: readonly number[];
  color: string;
}) {
  return (
    <div className="flex flex-col items-center gap-1">
      <span className="font-display text-muted-foreground text-xs font-semibold tracking-widest uppercase">{title}</span>
      <svg viewBox="-36 0 272 200" className="w-full max-w-64" aria-hidden="true">
        {RINGS.map((r) => (
          <polygon key={r} points={polygon([r, r, r, r])} fill="none" stroke="var(--border)" strokeWidth={1} />
        ))}
        {ANGLES.map((angle) => {
          const [x, y] = coords(angle, 1);
          return <line key={angle} x1={CENTER} y1={CENTER} x2={x} y2={y} stroke="var(--border)" />;
        })}
        <polygon points={polygon(shape, 0.1)} fill={color} fillOpacity={0.08} stroke={color} strokeOpacity={0.35} strokeDasharray="3 3" />
        <polygon
          points={polygon(shape)}
          fill={color}
          fillOpacity={0.28}
          stroke={color}
          strokeWidth={2}
          strokeLinejoin="round"
          className="motion-safe:animate-in motion-safe:fade-in motion-safe:zoom-in-90 origin-center motion-safe:duration-700"
        />
        {axes.map((axis, i) => {
          const pos = LABEL_POS[i];
          return pos === undefined ? null : (
            <text key={axis} x={pos.x} y={pos.y} textAnchor={pos.anchor} fontSize={11} fill="var(--muted-foreground)">
              {axis}
            </text>
          );
        })}
      </svg>
    </div>
  );
}

const TREND_W = 320;
const TREND_H = 96;

function trendPath(values: readonly number[], offset = 0): string {
  const step = TREND_W / (values.length - 1);
  return values
    .map((v, i) => `${i === 0 ? "M" : "L"}${(i * step).toFixed(1)},${(TREND_H - (v + offset) * TREND_H).toFixed(1)}`)
    .join(" ");
}

function bandPath(values: readonly number[], spread: number): string {
  const step = TREND_W / (values.length - 1);
  const upper = values.map((v, i) => `${(i * step).toFixed(1)},${(TREND_H - (v + spread) * TREND_H).toFixed(1)}`);
  const lower = values
    .map((v, i) => `${(i * step).toFixed(1)},${(TREND_H - (v - spread) * TREND_H).toFixed(1)}`)
    .reverse();
  return `M${upper.join(" L")} L${lower.join(" L")} Z`;
}

function Trend({ title, legend }: { title: string; legend: readonly [string, string] }) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3">
        <span className="font-display text-muted-foreground text-xs font-semibold tracking-widest uppercase">{title}</span>
        <span className="text-muted-foreground flex items-center gap-3 text-xs">
          <span className="flex items-center gap-1.5">
            <span className="bg-osu-pink size-2 rounded-full" aria-hidden="true" />
            {legend[0]}
          </span>
          <span className="flex items-center gap-1.5">
            <span className="bg-osu-blue size-2 rounded-full" aria-hidden="true" />
            {legend[1]}
          </span>
        </span>
      </div>
      <svg viewBox={`0 -8 ${TREND_W} ${TREND_H + 16}`} className="h-24 w-full" preserveAspectRatio="none" aria-hidden="true">
        {[0.25, 0.5, 0.75].map((y) => (
          <line key={y} x1={0} x2={TREND_W} y1={TREND_H * y} y2={TREND_H * y} stroke="var(--border)" strokeDasharray="2 4" />
        ))}
        <path d={bandPath(TREND_A, 0.07)} fill="var(--osu-pink)" fillOpacity={0.12} />
        <path d={bandPath(TREND_B, 0.07)} fill="var(--osu-blue)" fillOpacity={0.12} />
        <path d={trendPath(TREND_A)} fill="none" stroke="var(--osu-pink)" strokeWidth={2} vectorEffect="non-scaling-stroke" />
        <path d={trendPath(TREND_B)} fill="none" stroke="var(--osu-blue)" strokeWidth={2} vectorEffect="non-scaling-stroke" />
      </svg>
    </div>
  );
}

export function SkillPreview() {
  const { t } = useTranslation();
  const axis = (id: string) => t(`roadmap.axis.${id}`);
  return (
    <div role="img" aria-label={t("roadmap.skill.previewLabel")} className="flex flex-col gap-5">
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        <Radar title={t("roadmap.skill.rice")} axes={RICE_AXES.map(axis)} shape={RICE_SHAPE} color="var(--osu-pink)" />
        <Radar title={t("roadmap.skill.ln")} axes={LN_AXES.map(axis)} shape={LN_SHAPE} color="var(--osu-blue)" />
      </div>
      <Trend title={t("roadmap.skill.trend")} legend={[t("roadmap.skill.rice"), t("roadmap.skill.ln")]} />
    </div>
  );
}
