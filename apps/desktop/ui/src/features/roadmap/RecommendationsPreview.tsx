import { useTranslation } from "react-i18next";

// Illustrative cards only: chart names stay as skeleton bars so nothing reads as a real pick.
const MOCK_PICKS = [
  { axis: "regular_speed", rate: "×1.10", from: 0.38, to: 0.56, tint: "var(--osu-pink)" },
  { axis: "regular_tech", rate: "×1.00", from: 0.12, to: 0.3, tint: "var(--osu-purple)" },
  { axis: "ln_inverse", rate: "×0.95", from: 0.62, to: 0.84, tint: "var(--osu-blue)" },
] as const;

function Skeleton({ className }: { className: string }) {
  return <span aria-hidden="true" className={`bg-foreground/10 block h-2 rounded-full ${className}`} />;
}

export function RecommendationsPreview() {
  const { t } = useTranslation();
  return (
    <div role="img" aria-label={t("roadmap.recommendations.previewLabel")} className="flex flex-col gap-3">
      {MOCK_PICKS.map((pick, index) => (
        <div
          key={pick.axis}
          className="bg-surface-raised/80 ring-border motion-safe:animate-in motion-safe:fade-in motion-safe:slide-in-from-bottom-2 flex items-center gap-4 overflow-hidden rounded-lg p-3 ring-1 motion-safe:fill-mode-both motion-safe:duration-500"
          style={{ animationDelay: `${index * 120}ms` }}
        >
          <div
            aria-hidden="true"
            className="size-14 shrink-0 rounded-md"
            style={{ background: `linear-gradient(135deg, color-mix(in oklch, ${pick.tint} 55%, transparent), var(--card))` }}
          />
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <div className="flex flex-col gap-1.5">
              <Skeleton className="w-3/5" />
              <Skeleton className="w-2/5 opacity-70" />
            </div>
            <div className="flex items-center gap-2">
              <span className="text-muted-foreground w-14 shrink-0 text-[0.65rem] tracking-wider uppercase">
                {t("roadmap.recommendations.section")}
              </span>
              <span className="bg-foreground/10 relative h-1.5 flex-1 rounded-full">
                <span
                  className="absolute inset-y-0 rounded-full"
                  style={{ left: `${pick.from * 100}%`, right: `${(1 - pick.to) * 100}%`, background: pick.tint }}
                />
              </span>
            </div>
          </div>
          <div className="flex shrink-0 flex-col items-end gap-1.5">
            <span className="rounded-full border px-2 py-0.5 text-xs font-medium" style={{ color: pick.tint, borderColor: pick.tint }}>
              {t(`roadmap.axis.${pick.axis}`)}
            </span>
            <span className="tabular font-display text-sm font-semibold">
              <span className="sr-only">{t("roadmap.recommendations.rate")} </span>
              {pick.rate}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}
