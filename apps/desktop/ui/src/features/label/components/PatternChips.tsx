import { useTranslation } from "react-i18next";
import type { PatternDefDto } from "@/ipc/bindings";
import { cn } from "@/shared/lib/utils";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/shared/ui/tooltip";

// Hover help should not pop on every pass of the mouse across the chip grid.
const TOOLTIP_DELAY_MS = 400;

interface AxisGroup {
  axis: string;
  patterns: PatternDefDto[];
}

/** Taxonomy order is meaningful (the CLI legend follows it), so groups keep first-seen order. */
function groupByAxis(taxonomy: readonly PatternDefDto[]): AxisGroup[] {
  const groups: AxisGroup[] = [];
  for (const pattern of taxonomy) {
    const group = groups.find((g) => g.axis === pattern.axis);
    if (group === undefined) {
      groups.push({ axis: pattern.axis, patterns: [pattern] });
    } else {
      group.patterns.push(pattern);
    }
  }
  return groups;
}

/** `7k.regular.jack` → `regular_jack`: the keymode prefix does not change the axis name. */
function axisKey(axis: string): string {
  return axis.split(".").slice(1).join("_");
}

export function patternName(id: string): string {
  return (id.split(".").at(-1) ?? id).replaceAll("_", " ");
}

interface PatternChipsProps {
  taxonomy: readonly PatternDefDto[];
  isActive: (pattern: PatternDefDto) => boolean;
  onToggle: (pattern: PatternDefDto) => void;
}

export function PatternChips({ taxonomy, isActive, onToggle }: PatternChipsProps) {
  const { t } = useTranslation();
  return (
    <TooltipProvider delayDuration={TOOLTIP_DELAY_MS}>
      <div className="flex flex-col gap-3">
        {groupByAxis(taxonomy).map(({ axis, patterns }) => {
          const key = axisKey(axis);
          return (
            <section key={axis} aria-label={t(`label.axis.${key}`, { defaultValue: key })} className="flex flex-col gap-1.5">
              <h4 className="text-muted-foreground text-xs font-medium tracking-wide uppercase">
                {t(`label.axis.${key}`, { defaultValue: key })}
              </h4>
              <div className="flex flex-wrap gap-1.5">
                {patterns.map((pattern) => {
                  const active = isActive(pattern);
                  return (
                    <Tooltip key={pattern.id}>
                      <TooltipTrigger asChild>
                        <button
                          type="button"
                          aria-pressed={active}
                          onClick={() => {
                            onToggle(pattern);
                          }}
                          className={cn(
                            "inline-flex h-7 items-center gap-1.5 rounded-md border px-2 text-xs transition-colors outline-none",
                            "focus-visible:ring-ring/50 focus-visible:ring-3",
                            active
                              ? "border-primary bg-primary text-primary-foreground"
                              : "border-border bg-background hover:bg-muted",
                          )}
                        >
                          <kbd className="font-mono font-semibold">{pattern.key}</kbd>{" "}
                          <span>{patternName(pattern.id)}</span>
                        </button>
                      </TooltipTrigger>
                      <TooltipContent side="left">{pattern.description}</TooltipContent>
                    </Tooltip>
                  );
                })}
              </div>
            </section>
          );
        })}
      </div>
    </TooltipProvider>
  );
}
