import { useTranslation } from "react-i18next";
import type { TrendPointDto } from "@/ipc/bindings";
import { usePreviewFormat } from "../format";
import { trendGeometry } from "../model";

const W = 320;
const H = 96;
const PAD = 6;

export function TrendChart({ trend }: { trend: readonly TrendPointDto[] }) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  const first = trend[0];
  const last = trend.at(-1);
  if (first === undefined || last === undefined) {
    return <p className="text-muted-foreground text-sm">{t("preview.trend.empty")}</p>;
  }
  const geometry = trendGeometry(trend, { width: W, height: H });
  const label =
    trend.length === 1
      ? t("preview.trend.single", { month: format.month(first.month), value: format.approx(first.overallCenti) })
      : t("preview.trend.label", {
          from: format.month(first.month),
          fromValue: format.approx(first.overallCenti),
          to: format.month(last.month),
          toValue: format.approx(last.overallCenti),
        });
  const path = geometry.points.map((p, i) => `${i === 0 ? "M" : "L"}${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" ");
  const area = `${path} L${W},${H} L0,${H} Z`;
  return (
    <div role="img" aria-label={label} className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-2 gap-y-1">
      <div aria-hidden="true" className="text-muted-foreground flex flex-col justify-between py-0.5 text-right text-[0.7rem] tabular-nums">
        <span>{format.plain(geometry.hi)}</span>
        {geometry.hi !== geometry.lo && <span>{format.plain(geometry.lo)}</span>}
      </div>
      <svg viewBox={`${-PAD} ${-PAD} ${W + 2 * PAD} ${H + 2 * PAD}`} preserveAspectRatio="none" className="h-24 w-full" aria-hidden="true">
        <line x1={0} x2={W} y1={0} y2={0} stroke="var(--border)" strokeDasharray="2 4" vectorEffect="non-scaling-stroke" />
        <line x1={0} x2={W} y1={H} y2={H} stroke="var(--border)" vectorEffect="non-scaling-stroke" />
        {trend.length > 1 && (
          <>
            <path d={area} fill="var(--osu-pink)" fillOpacity={0.1} />
            <path d={path} fill="none" stroke="var(--osu-pink)" strokeWidth={2} strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
          </>
        )}
        {geometry.points.map((p) => (
          <line
            key={p.month}
            x1={p.x}
            x2={p.x}
            y1={p.y}
            y2={p.y}
            stroke="var(--osu-pink)"
            strokeWidth={7}
            strokeLinecap="round"
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </svg>
      <span />
      <div className="text-muted-foreground flex justify-between text-[0.7rem]">
        <span>{format.month(first.month)}</span>
        {trend.length > 1 && <span>{format.month(last.month)}</span>}
      </div>
    </div>
  );
}
