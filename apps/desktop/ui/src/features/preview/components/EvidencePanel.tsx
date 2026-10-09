import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { EvidenceDto, EvidenceTierDto } from "@/ipc/bindings";
import { cn } from "@/shared/lib/utils";
import { usePreviewFormat } from "../format";
import { Heading, type HeadingLevel } from "./Heading";

const TIER_TONE: Record<EvidenceTierDto, string> = {
  low: "bg-destructive/15 text-destructive",
  medium: "bg-osu-yellow/15 text-osu-yellow",
  ok: "bg-success/15 text-success",
};

const TIER_FILL: Record<EvidenceTierDto, number> = { low: 1, medium: 2, ok: 3 };

export function EvidencePanel({ evidence, level }: { evidence: EvidenceDto; level: HeadingLevel }) {
  const { t, i18n } = useTranslation();
  const format = usePreviewFormat();
  const headingId = useId();
  const reason = (code: string) => {
    const key = `preview.evidence.reason.${code}`;
    return i18n.exists(key) ? t(key) : code;
  };
  return (
    <section aria-labelledby={headingId} className="bg-card ring-border flex flex-col gap-4 rounded-xl p-5 shadow-lg shadow-black/20 ring-1">
      <Heading level={level} id={headingId}>
        {t("preview.evidence.title")}
      </Heading>
      <div className="flex flex-col gap-2">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <span className="font-display text-xl font-bold tabular-nums">{t("preview.evidence.counted", { count: evidence.counted })}</span>
          <span className={cn("inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium", TIER_TONE[evidence.tier])}>
            <span aria-hidden="true" className="flex gap-0.5">
              {[1, 2, 3].map((step) => (
                <span key={step} className={cn("h-2 w-1 rounded-full bg-current", step > TIER_FILL[evidence.tier] && "opacity-25")} />
              ))}
            </span>
            {t(`preview.evidence.tierName.${evidence.tier}`)}
          </span>
        </div>
        <p className="text-muted-foreground text-sm">{t(`preview.evidence.tier.${evidence.tier}`)}</p>
      </div>
      <div className="flex flex-col gap-2 border-t pt-3">
        <span className="text-muted-foreground text-xs font-medium">{t("preview.evidence.excluded")}</span>
        {evidence.excluded.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("preview.evidence.noneExcluded")}</p>
        ) : (
          <ul className="flex flex-col gap-1.5 text-sm">
            {evidence.excluded.map((e) => (
              <li key={e.reason} className="flex items-baseline justify-between gap-3">
                <span>{reason(e.reason)}</span>
                <span className="text-muted-foreground tabular-nums">{format.count(e.count)}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
