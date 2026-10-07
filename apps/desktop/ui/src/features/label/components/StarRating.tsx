import { Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { difficultyColour, starTextColour } from "../starColour";

/** The star rating as osu! badges it: a pill in the rating's difficulty colour. */
export function StarRating({ stars, className }: { stars: number; className?: string }) {
  const { t } = useTranslation();
  const shown = stars.toFixed(2);
  return (
    <span
      role="img"
      aria-label={t("label.header.stars", { stars: shown })}
      data-testid="star-rating"
      style={{ backgroundColor: difficultyColour(stars), color: starTextColour(stars) }}
      className={cn("inline-flex h-5 shrink-0 items-center gap-1 rounded-full px-2 text-xs font-bold tabular-nums", className)}
    >
      <Star aria-hidden className="size-3 fill-current" />
      {shown}
    </span>
  );
}
