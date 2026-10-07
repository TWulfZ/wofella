import { Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/shared/ui/badge";
import type { WindowOrigin } from "../session";
import { difficultyColour, starTextColour } from "../starColour";
import type { LabelWindow } from "../types";

function originKey(origin: WindowOrigin): string {
  switch (origin.kind) {
    case "plan":
      return "label.round";
    case "random":
      return "label.origin.random";
    case "nowPlaying":
      return origin.source === "osuWindow" ? "label.origin.osuWindow" : "label.origin.lastReplay";
  }
}

export interface HeaderBackground {
  mime: string;
  base64: string;
}

export interface ChartHeaderProps {
  window: LabelWindow;
  origin: WindowOrigin;
  /** null draws the fallback gradient; the header keeps its height either way, so nothing below it moves. */
  background: HeaderBackground | null;
}

function StarRating({ stars }: { stars: number }) {
  const { t } = useTranslation();
  const shown = stars.toFixed(2);
  return (
    <span
      role="img"
      aria-label={t("label.header.stars", { stars: shown })}
      data-testid="star-rating"
      style={{ backgroundColor: difficultyColour(stars), color: starTextColour(stars) }}
      className="inline-flex h-5 shrink-0 items-center gap-1 rounded-full px-2 text-xs font-bold tabular-nums"
    >
      <Star aria-hidden className="size-3 fill-current" />
      {shown}
    </span>
  );
}

export function ChartHeader({ window, origin, background }: ChartHeaderProps) {
  const { t } = useTranslation();
  return (
    <header className="relative isolate flex h-44 shrink-0 flex-col justify-end overflow-hidden rounded-xl border">
      {background === null ? (
        <div
          aria-hidden
          data-testid="header-fallback"
          className="from-osu-pink/35 via-osu-purple/25 to-osu-blue/35 absolute inset-0 -z-20 bg-linear-to-br"
        />
      ) : (
        <img
          src={`data:${background.mime};base64,${background.base64}`}
          alt=""
          draggable={false}
          // Scaled past the edges so the blur does not fade them into the border.
          className="absolute inset-0 -z-20 size-full scale-110 object-cover blur-sm"
        />
      )}
      {/* Darkens most where the text sits, so it keeps its contrast over any background. */}
      <div aria-hidden className="from-background via-background/85 to-background/40 absolute inset-0 -z-10 bg-linear-to-t" />
      <p className="bg-background/75 text-foreground absolute top-2 left-2 rounded-full px-2 py-0.5 text-[0.7rem] font-medium tracking-wide uppercase backdrop-blur-sm">
        {t(originKey(origin), { round: origin.kind === "plan" ? origin.round + 1 : 0 })}
      </p>
      <div className="flex min-w-0 flex-col gap-1 p-3">
        <h2 title={window.title} className="font-display line-clamp-2 text-xl leading-tight font-semibold break-words">
          {window.title}
        </h2>
        <p className="text-foreground/90 truncate text-sm">{window.artist}</p>
        <p className="text-foreground/75 truncate text-xs">{t("label.header.mappedBy", { creator: window.creator })}</p>
        <div className="mt-1 flex flex-wrap items-center gap-1.5">
          {window.stars !== null && <StarRating stars={window.stars} />}
          <Badge variant="secondary" className="max-w-full truncate">
            {window.version}
          </Badge>
          {window.level !== null && (
            <Badge variant="outline" className="bg-background/60">
              {t("label.level", { level: window.level })}
            </Badge>
          )}
          <Badge variant="outline" className="bg-background/60">
            {t("label.stratum", { stratum: window.stratum })}
          </Badge>
          <Badge variant={window.played ? "default" : "outline"} className={window.played ? undefined : "bg-background/60"}>
            {window.played ? t("label.played") : t("label.notPlayed")}
          </Badge>
        </div>
      </div>
    </header>
  );
}
