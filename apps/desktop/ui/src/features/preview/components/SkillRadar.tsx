import { useTranslation } from "react-i18next";
import type { SkillsetRatingDto } from "@/ipc/bindings";
import { usePreviewFormat } from "../format";
import { axisAngle, type RadarScale } from "../model";

const SIZE = 300;
const CENTER = SIZE / 2;
const RADIUS = 92;
const LABEL_GAP = 16;

function at(angle: number, r: number): [number, number] {
  return [CENTER + Math.cos(angle) * r, CENTER + Math.sin(angle) * r];
}

function anchorFor(angle: number): "start" | "middle" | "end" {
  const x = Math.cos(angle);
  if (Math.abs(x) < 0.2) {
    return "middle";
  }
  return x > 0 ? "start" : "end";
}

interface SkillRadarProps {
  skillsets: readonly SkillsetRatingDto[];
  unmeasured: ReadonlySet<string>;
  scale: RadarScale;
}

/** One axis per skillset the DTO sends; unmeasured axes keep their spoke but get no vertex. */
export function SkillRadar({ skillsets, unmeasured, scale }: SkillRadarProps) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  const n = skillsets.length;
  const span = scale.hi - scale.lo;
  const radiusOf = (centi: number) => (Math.min(Math.max(centi - scale.lo, 0), span) / span) * RADIUS;
  const ring = (r: number) =>
    skillsets
      .map((_, i) => at(axisAngle(i, n), r))
      .map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`)
      .join(" ");
  const measured = skillsets
    .map((s, i) => ({ s, i }))
    .filter(({ s }) => !unmeasured.has(s.id))
    .map(({ s, i }) => ({ id: s.id, point: at(axisAngle(i, n), radiusOf(s.ratingCenti)) }));
  const summary = skillsets
    .map((s) =>
      unmeasured.has(s.id)
        ? t("preview.radar.unmeasured", { name: format.skillset(s.id) })
        : t("preview.radar.value", { name: format.skillset(s.id), value: format.approx(s.ratingCenti) }),
    )
    .join(", ");

  return (
    <svg
      role="img"
      aria-label={t("preview.radar.label", { values: summary })}
      viewBox={`-40 -10 ${SIZE + 80} ${SIZE + 20}`}
      className="w-full max-w-sm overflow-visible"
    >
      {scale.rings.map((value) => (
        <polygon
          key={value}
          points={ring(radiusOf(value))}
          fill="none"
          stroke="var(--border)"
          strokeDasharray={value === scale.hi ? undefined : "2 4"}
        />
      ))}
      {scale.rings.map((value) => (
        <text
          key={value}
          x={CENTER + 4}
          y={CENTER - radiusOf(value) - 3}
          fontSize={9}
          fill="var(--muted-foreground)"
          aria-hidden="true"
        >
          {format.plain(value)}
        </text>
      ))}
      {skillsets.map((s, i) => {
        const angle = axisAngle(i, n);
        const off = unmeasured.has(s.id);
        const [x, y] = at(angle, RADIUS);
        const [lx, ly] = at(angle, RADIUS + LABEL_GAP);
        return (
          <g key={s.id} data-axis={s.id} opacity={off ? 0.45 : 1}>
            <line x1={CENTER} y1={CENTER} x2={x} y2={y} stroke="var(--border)" strokeDasharray={off ? "3 3" : undefined} />
            <text
              x={lx}
              y={ly}
              dy="0.35em"
              textAnchor={anchorFor(angle)}
              fontSize={11}
              fontWeight={600}
              fill={off ? "var(--muted-foreground)" : "var(--foreground)"}
            >
              {format.skillset(s.id)}
            </text>
            {off && (
              <text x={lx} y={ly + 13} dy="0.35em" textAnchor={anchorFor(angle)} fontSize={9} fill="var(--muted-foreground)">
                {t("preview.radar.notMeasured")}
              </text>
            )}
          </g>
        );
      })}
      {measured.length >= 3 && (
        <polygon
          points={measured.map(({ point: [x, y] }) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ")}
          fill="var(--osu-pink)"
          fillOpacity={0.22}
          stroke="var(--osu-pink)"
          strokeWidth={2}
          strokeLinejoin="round"
          className="motion-safe:animate-in motion-safe:fade-in motion-safe:zoom-in-75 origin-center motion-safe:duration-700"
        />
      )}
      {measured.map(({ id, point: [x, y] }) => (
        <circle key={id} cx={x} cy={y} r={3.5} fill="var(--osu-pink)" stroke="var(--card)" strokeWidth={1.5} />
      ))}
    </svg>
  );
}
