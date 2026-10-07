import { useId } from "react";
import { useTranslation } from "react-i18next";
import { AxisIcon, axisKey, familyAccent, patternName } from "@/features/label";
import { cn } from "@/shared/lib/utils";
import type { AxisBar, FamilyBars } from "../model";
import { QueryAlert } from "./QueryAlert";

function share(count: number, max: number): string {
  return `${max === 0 ? 0 : (count / max) * 100}%`;
}

function Bar({ count, max, className }: { count: number; max: number; className: string }) {
  return (
    <span aria-hidden="true" className="bg-muted relative h-2 flex-1 overflow-hidden rounded-full">
      <span
        style={{ width: share(count, max) }}
        className={cn("absolute inset-y-0 left-0 rounded-full motion-safe:transition-[width] motion-safe:duration-300", className)}
      />
    </span>
  );
}

function AxisRow({ bar, max, patternMax, accentBar }: { bar: AxisBar; max: number; patternMax: number; accentBar: string }) {
  const { t } = useTranslation();
  const key = axisKey(bar.axis);
  const name = t(`label.axis.${key}`, { defaultValue: key });
  return (
    <li aria-label={t("labelProgress.axes.value", { name, count: bar.count })} className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2 text-sm">
        <AxisIcon axis={bar.axis} />
        <span className="w-24 shrink-0 truncate font-medium">{name}</span>
        <Bar count={bar.count} max={max} className={accentBar} />
        <span className="w-8 shrink-0 text-right tabular-nums">{bar.count}</span>
      </div>
      <ul className="flex flex-col gap-1 pl-7">
        {bar.patterns.map((pattern) => {
          const patternLabel = patternName(pattern.id);
          return (
            <li
              key={pattern.id}
              aria-label={t("labelProgress.axes.value", { name: patternLabel, count: pattern.count })}
              className="text-muted-foreground flex items-center gap-2 text-xs"
            >
              <span className="w-24 shrink-0 truncate capitalize">{patternLabel}</span>
              <Bar count={pattern.count} max={patternMax} className={cn(accentBar, "opacity-70")} />
              <span className="w-8 shrink-0 text-right tabular-nums">{pattern.count}</span>
            </li>
          );
        })}
      </ul>
    </li>
  );
}

function FamilySection({ family, max, patternMax }: { family: FamilyBars; max: number; patternMax: number }) {
  const { t } = useTranslation();
  const headingId = useId();
  const accent = familyAccent(family.family);
  return (
    <section aria-labelledby={headingId} className={cn("flex flex-col gap-3", accent.icon)}>
      <h3 id={headingId} className={cn("font-display text-sm font-black tracking-[0.2em] uppercase", accent.heading)}>
        {t(`label.patternGrid.family.${family.family}`, { defaultValue: family.family.toUpperCase() })}
      </h3>
      <ul className="flex flex-col gap-3">
        {family.axes.map((bar) => (
          <AxisRow key={bar.axis} bar={bar} max={max} patternMax={patternMax} accentBar={accent.bar} />
        ))}
      </ul>
    </section>
  );
}

interface AxisBarsProps {
  families: FamilyBars[] | undefined;
  noPattern: number | undefined;
  /** The progress failed; its alert and retry live in the totals. */
  unavailable: boolean;
  taxonomyFailure?: { error: unknown; retry: () => void } | undefined;
}

export function AxisBars({ families, noPattern, unavailable, taxonomyFailure }: AxisBarsProps) {
  const { t } = useTranslation();
  const headingId = useId();
  const axes = families?.flatMap((f) => f.axes) ?? [];
  const max = Math.max(0, ...axes.map((a) => a.count));
  const patternMax = Math.max(0, ...axes.flatMap((a) => a.patterns.map((p) => p.count)));
  return (
    <section aria-labelledby={headingId} className="bg-card ring-border flex flex-col gap-4 rounded-xl p-5 shadow-lg shadow-black/20 ring-1">
      <div className="flex flex-col gap-1">
        <h2 id={headingId} className="font-display text-base font-semibold">
          {t("labelProgress.axes.title")}
        </h2>
        <p className="text-muted-foreground text-sm">{t("labelProgress.axes.description")}</p>
      </div>
      {taxonomyFailure !== undefined ? (
        <QueryAlert error={taxonomyFailure.error} onRetry={taxonomyFailure.retry} />
      ) : families === undefined ? (
        <p className="text-muted-foreground text-sm">{t(unavailable ? "labelProgress.unavailable" : "common.loading")}</p>
      ) : (
        <>
          {families.map((family) => (
            <FamilySection key={family.family} family={family} max={max} patternMax={patternMax} />
          ))}
          {noPattern !== undefined && (
            <p className="text-muted-foreground border-t pt-3 text-sm">{t("labelProgress.axes.noPattern", { count: noPattern })}</p>
          )}
        </>
      )}
    </section>
  );
}
