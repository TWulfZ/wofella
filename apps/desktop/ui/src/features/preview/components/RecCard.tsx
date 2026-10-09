import { Copy, Play, Sparkles, WandSparkles } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { RecItemDto, ReasonDto } from "@/ipc/bindings";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";
import { type PreviewFormat, usePreviewFormat } from "../format";
import { bandFraction, osuSearchText, type RateKind, rateKind } from "../model";

export const REC_CARD_PARAMS = {
  copiedFeedbackMs: 1800,
} as const;

const RATE_TONE: Record<RateKind, string> = {
  nm: "text-foreground bg-muted/50",
  ht: "text-osu-blue bg-osu-blue/10",
  dt: "text-osu-pink bg-osu-pink/10",
  custom: "text-osu-purple bg-osu-purple/10",
};

const MOD_LABEL: Partial<Record<RateKind, string>> = { ht: "preview.recs.rate.ht", dt: "preview.recs.rate.dt" };

function useReasonText(format: PreviewFormat) {
  const { t, i18n } = useTranslation();
  const centi = (raw: string | undefined) => {
    const n = Number(raw);
    return raw === undefined || Number.isNaN(n) ? (raw ?? "") : format.approx(n);
  };
  const rate = (raw: string | undefined) => {
    const n = Number(raw);
    return raw === undefined || Number.isNaN(n) ? (raw ?? "") : format.rate(n);
  };
  return ({ code, args }: ReasonDto) => {
    const key = `preview.recs.reasons.${code}`;
    if (!i18n.exists(key)) {
      return code;
    }
    switch (code) {
      case "deficit":
        return t(key, { skillset: format.skillset(args[0] ?? ""), rating: centi(args[1]) });
      case "push":
        return t(key, { target: centi(args[0]) });
      case "skillset":
        return t(key, { skillset: format.skillset(args[0] ?? "") });
      case "needs_rate_copy":
        return t(key, { rate: rate(args[0]) });
      default:
        return t(key);
    }
  };
}

function RateBlock({ item }: { item: RecItemDto }) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  const kind = rateKind(item.rateMilli);
  const mod = MOD_LABEL[kind];
  return (
    <div className={cn("relative flex flex-col items-center justify-center gap-1.5 px-2 py-4 text-center", RATE_TONE[kind])}>
      <span aria-hidden="true" className="absolute inset-y-0 left-0 w-1 bg-current opacity-70" />
      <span className="sr-only">{t("preview.recs.rate.label")}: </span>
      <span data-testid="rate" className="font-display flex flex-col items-center leading-none font-extrabold italic tabular-nums">
        <span className="text-2xl tracking-tight">{format.rate(item.rateMilli)}</span>
        {mod !== undefined && (
          <>
            {" "}
            <span className="mt-1 text-sm tracking-wide not-italic">{t(mod)}</span>
          </>
        )}
      </span>
      {item.isRateCopy && (
        <span className="border-osu-purple/40 text-osu-purple rounded-full border px-1.5 py-0.5 text-[0.65rem] leading-none font-medium">
          {t("preview.recs.rateCopy")}
        </span>
      )}
    </div>
  );
}

/** The band is the same on every card, so the chart dots line up across the list and read as relative difficulty. */
function BandMeter({ focusCenti, ratingCenti, band }: { focusCenti: number; ratingCenti: number; band: readonly [number, number] }) {
  const at = (centi: number) => `${String(bandFraction(centi, band) * 100)}%`;
  return (
    <span aria-hidden="true" className="bg-muted relative block h-1.5 w-full rounded-full">
      <span className="bg-foreground/50 absolute -top-1 h-3.5 w-0.5 -translate-x-1/2 rounded-full" style={{ left: at(ratingCenti) }} />
      <span
        className="bg-osu-pink ring-card absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2"
        style={{ left: at(focusCenti) }}
      />
    </span>
  );
}

function CopyName({ item }: { item: RecItemDto }) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<"copied" | "copyFailed" | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (timer.current !== null) {
        clearTimeout(timer.current);
      }
    },
    [],
  );
  const copy = async (): Promise<void> => {
    let next: "copied" | "copyFailed" = "copied";
    try {
      await navigator.clipboard.writeText(osuSearchText(item));
    } catch {
      next = "copyFailed";
    }
    setStatus(next);
    if (timer.current !== null) {
      clearTimeout(timer.current);
    }
    timer.current = setTimeout(() => {
      setStatus(null);
    }, REC_CARD_PARAMS.copiedFeedbackMs);
  };
  return (
    <>
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => {
          void copy();
        }}
      >
        <Copy aria-hidden="true" />
        {t("preview.recs.actions.copyName")}
      </Button>
      <span role="status" className="text-muted-foreground text-xs">
        {status === null ? "" : t(`preview.recs.actions.${status}`)}
      </span>
    </>
  );
}

function GenerateRateCopy({ item, onGenerate }: { item: RecItemDto; onGenerate: ((item: RecItemDto) => void) | undefined }) {
  const { t } = useTranslation();
  const label = t("preview.recs.actions.generate");
  if (onGenerate !== undefined) {
    return (
      <Button
        type="button"
        size="sm"
        data-testid="generate-rate-copy"
        onClick={() => {
          onGenerate(item);
        }}
      >
        <WandSparkles aria-hidden="true" />
        {label}
      </Button>
    );
  }
  // A disabled button gets no pointer events, so the focusable wrapper carries the tooltip.
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span tabIndex={0} className="focus-visible:ring-ring inline-flex rounded-lg focus-visible:ring-2 focus-visible:outline-none">
          <Button type="button" size="sm" disabled data-testid="generate-rate-copy">
            <WandSparkles aria-hidden="true" />
            {label}
          </Button>
        </span>
      </TooltipTrigger>
      <TooltipContent side="top">{t("preview.recs.actions.generateSoon")}</TooltipContent>
    </Tooltip>
  );
}

interface RecCardProps {
  item: RecItemDto;
  focus: string;
  ratingCenti: number;
  band: readonly [number, number];
  onGenerate: ((item: RecItemDto) => void) | undefined;
}

export function RecCard({ item, focus, ratingCenti, band, onGenerate }: RecCardProps) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  const reasonText = useReasonText(format);
  return (
    <li className="bg-card ring-border grid grid-cols-[5.5rem_minmax(0,1fr)] overflow-hidden rounded-xl shadow-lg shadow-black/20 ring-1">
      <RateBlock item={item} />
      <div className="flex min-w-0 flex-col gap-3 p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 flex-col gap-0.5">
            <h2 className="font-display truncate text-base leading-snug font-semibold" title={item.title}>
              {item.title}
            </h2>
            <p className="text-muted-foreground truncate text-sm">
              {item.artist} · {t("preview.recs.by", { creator: item.creator })}
            </p>
            <p className="truncate text-sm">[{item.version}]</p>
          </div>
          <span
            className={cn(
              "inline-flex shrink-0 items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium",
              item.played ? "bg-muted text-muted-foreground" : "bg-success/15 text-success",
            )}
          >
            {item.played ? <Play aria-hidden="true" className="size-3" /> : <Sparkles aria-hidden="true" className="size-3" />}
            {item.played ? t("preview.recs.played") : t("preview.recs.unplayed")}
          </span>
        </div>
        <div className="flex flex-col gap-2">
          <BandMeter focusCenti={item.focusCenti} ratingCenti={ratingCenti} band={band} />
          <p className="flex flex-wrap gap-x-4 gap-y-1 text-sm tabular-nums">
            <span className="text-osu-pink font-semibold">
              {t("preview.recs.focusValue", { skillset: format.skillset(focus), value: format.approx(item.focusCenti) })}
            </span>
            {focus !== "overall" && (
              <span className="text-muted-foreground">
                {t("preview.recs.focusValue", { skillset: format.skillset("overall"), value: format.approx(item.overallCenti) })}
              </span>
            )}
          </p>
        </div>
        <div aria-label={t("preview.recs.reasons.label")} role="group" className="text-muted-foreground flex flex-col gap-1 text-sm">
          {item.reasons.map((reason) => (
            <p key={reason.code} className="before:bg-osu-pink/60 relative pl-3 before:absolute before:top-2 before:left-0 before:size-1.5 before:rounded-full">
              {reasonText(reason)}
            </p>
          ))}
        </div>
        <div className="flex flex-wrap items-center gap-2 border-t pt-3">
          <CopyName item={item} />
          {item.needsRateCopy && (
            <span className="ml-auto">
              <GenerateRateCopy item={item} onGenerate={onGenerate} />
            </span>
          )}
        </div>
      </div>
    </li>
  );
}
