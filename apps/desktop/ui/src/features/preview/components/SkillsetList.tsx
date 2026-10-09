import { useTranslation } from "react-i18next";
import type { SkillsetRatingDto } from "@/ipc/bindings";
import { cn } from "@/shared/lib/utils";
import { usePreviewFormat } from "../format";

interface SkillsetListProps {
  skillsets: readonly SkillsetRatingDto[];
  unmeasured: ReadonlySet<string>;
  /** The radar's outer ring, so bar lengths and the radar read on one scale. */
  maxCenti: number;
}

export function SkillsetList({ skillsets, unmeasured, maxCenti }: SkillsetListProps) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  const strongest = Math.max(...skillsets.filter((s) => !unmeasured.has(s.id)).map((s) => s.ratingCenti));
  return (
    <ul aria-label={t("preview.skillsets.title")} className="flex flex-col gap-2.5">
      {skillsets.map((s) => {
        const off = unmeasured.has(s.id);
        return (
          <li key={s.id} className="grid grid-cols-[7rem_minmax(0,1fr)_4.5rem] items-center gap-3 text-sm">
            <span className={cn("truncate", off && "text-muted-foreground")}>{format.skillset(s.id)}</span>
            <span aria-hidden="true" className="bg-muted relative h-1.5 overflow-hidden rounded-full">
              {!off && (
                <span
                  className={cn("absolute inset-y-0 left-0 rounded-full", s.ratingCenti === strongest ? "bg-osu-pink" : "bg-osu-pink/50")}
                  style={{ width: `${String(Math.min(100, (s.ratingCenti / maxCenti) * 100))}%` }}
                />
              )}
            </span>
            <span className={cn("text-right tabular-nums", off ? "text-muted-foreground text-xs" : "font-semibold")}>
              {off ? t("preview.skillsets.notMeasured") : format.approx(s.ratingCenti)}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
