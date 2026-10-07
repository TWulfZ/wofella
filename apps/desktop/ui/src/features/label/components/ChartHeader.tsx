import { Check, Maximize2, SkipForward, Star, Trophy, Undo2 } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/shared/ui/dialog";
import { formatMinSec } from "../format";
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

export function backgroundDataUrl(background: HeaderBackground | null | undefined): string | null {
  return background == null ? null : `data:${background.mime};base64,${background.base64}`;
}

/** Structural mirror of `ChartDetailsDto`; fields the catalog lacks arrive as null or empty. */
export interface ChartDetails {
  md5: string;
  title: string;
  artist: string;
  creator: string;
  version: string;
  source: string | null;
  tags: string[];
  stars: number | null;
  od: number | null;
  hp: number | null;
  lengthMs: number;
  bpmMin: number | null;
  bpmMax: number | null;
  nNotes: number;
  nLn: number;
  setId: number | null;
  beatmapId: number | null;
}

export interface SessionCounters {
  labelled: number;
  skipped: number;
  undone: number;
  gold: number;
}

export interface ChartHeaderProps {
  window: LabelWindow;
  origin?: WindowOrigin | undefined;
  /** A data URL; null or absent draws the fallback gradient in the same box, so nothing below it moves. */
  background?: string | null | undefined;
  details?: ChartDetails | undefined;
  nav: ReactNode;
  counters: SessionCounters;
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

/** Drops float noise from the f32 IPC values (7.5 stays 7.5, 7.4999 becomes 7.5). */
function decimal(value: number): string {
  return String(Math.round(value * 10) / 10);
}

function bpmText(min: number | null, max: number | null): string | null {
  if (min === null || max === null) {
    return min === null && max === null ? null : decimal(min ?? max ?? 0);
  }
  return decimal(min) === decimal(max) ? decimal(min) : `${decimal(min)}–${decimal(max)}`;
}

const COUNTERS = [
  { key: "labelled", icon: Check },
  { key: "skipped", icon: SkipForward },
  { key: "undone", icon: Undo2 },
  { key: "gold", icon: Trophy },
] as const;

function Counters({ counters }: { counters: SessionCounters }) {
  const { t } = useTranslation();
  return (
    <ul aria-label={t("label.header.counters.label")} className="text-foreground/80 ml-auto flex shrink-0 items-center gap-2.5 text-xs tabular-nums">
      {COUNTERS.map(({ key, icon: Icon }) => {
        const text = t(`label.header.counters.${key}`, { count: counters[key] });
        return (
          <li key={key} title={text} className="inline-flex items-center gap-1">
            <Icon aria-hidden className="text-muted-foreground size-3.5" />
            <span aria-hidden>{counters[key]}</span>
            <span className="sr-only">{text}</span>
          </li>
        );
      })}
    </ul>
  );
}

function DetailRow({ term, children }: { term: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-muted-foreground">{term}</dt>
      <dd className="min-w-0 break-words">{children}</dd>
    </>
  );
}

function DetailsDialog({ window, background, details }: Pick<ChartHeaderProps, "window" | "background" | "details">) {
  const { t } = useTranslation();
  const title = details?.title ?? window.title;
  const stars = details?.stars ?? window.stars;
  const bpm = details === undefined ? null : bpmText(details.bpmMin, details.bpmMax);
  const source = details?.source?.trim() ?? "";
  const label = t("label.header.details");
  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={label}
          title={label}
          className="bg-background/70 hover:bg-background/90 backdrop-blur-sm"
        >
          <Maximize2 aria-hidden />
        </Button>
      </DialogTrigger>
      <DialogContent closeLabel={t("common.close")} className="max-h-[90dvh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader className="pr-8">
          <DialogTitle className="font-display text-lg leading-tight font-semibold">{title}</DialogTitle>
          <DialogDescription>{details?.artist ?? window.artist}</DialogDescription>
        </DialogHeader>
        {background != null && (
          <img
            src={background}
            alt={t("label.header.imageAlt", { title })}
            draggable={false}
            className="bg-background max-h-[55dvh] w-full rounded-lg object-contain"
          />
        )}
        <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1.5 text-sm">
          <DetailRow term={t("label.header.field.mapper")}>{details?.creator ?? window.creator}</DetailRow>
          <DetailRow term={t("label.header.field.difficulty")}>{details?.version ?? window.version}</DetailRow>
          {stars !== null && (
            <DetailRow term={t("label.header.field.stars")}>
              <StarRating stars={stars} />
            </DetailRow>
          )}
          {details !== undefined && (
            <>
              <DetailRow term={t("label.header.field.length")}>
                <span className="tabular-nums">{formatMinSec(details.lengthMs)}</span>
              </DetailRow>
              {bpm !== null && (
                <DetailRow term={t("label.header.field.bpm")}>
                  <span className="tabular-nums">{bpm}</span>
                </DetailRow>
              )}
              {details.od !== null && <DetailRow term={t("label.header.field.od")}>{decimal(details.od)}</DetailRow>}
              {details.hp !== null && <DetailRow term={t("label.header.field.hp")}>{decimal(details.hp)}</DetailRow>}
              <DetailRow term={t("label.header.field.notes")}>
                <span className="tabular-nums">{details.nNotes}</span>
              </DetailRow>
              <DetailRow term={t("label.header.field.longNotes")}>
                <span className="tabular-nums">{details.nLn}</span>
              </DetailRow>
              {source !== "" && <DetailRow term={t("label.header.field.source")}>{source}</DetailRow>}
              {details.tags.length > 0 && (
                <DetailRow term={t("label.header.field.tags")}>
                  <ul className="flex flex-wrap gap-1">
                    {/* Mappers repeat words in the tag string; the tag doubles as the key, so it must be unique. */}
                    {[...new Set(details.tags)].map((tag) => (
                      <li key={tag} className="bg-muted rounded-full px-2 text-xs leading-5">
                        {tag}
                      </li>
                    ))}
                  </ul>
                </DetailRow>
              )}
              {details.setId !== null && (
                <DetailRow term={t("label.header.field.setId")}>
                  <span className="font-mono tabular-nums">{details.setId}</span>
                </DetailRow>
              )}
              {details.beatmapId !== null && (
                <DetailRow term={t("label.header.field.beatmapId")}>
                  <span className="font-mono tabular-nums">{details.beatmapId}</span>
                </DetailRow>
              )}
            </>
          )}
          <DetailRow term={t("label.header.field.md5")}>
            <span className="font-mono text-xs break-all">{details?.md5 ?? window.anchor.md5}</span>
          </DetailRow>
        </dl>
      </DialogContent>
    </Dialog>
  );
}

export function ChartHeader({ window, origin, background = null, details, nav, counters }: ChartHeaderProps) {
  const { t } = useTranslation();
  return (
    // Full-bleed: the image is the panel's top background, so the card adds no frame or inset of its own.
    <header className="relative isolate flex w-full shrink-0 flex-col overflow-hidden rounded-t-xl border-b">
      {background === null ? (
        <div
          aria-hidden
          data-testid="header-fallback"
          className="from-osu-pink/35 via-osu-purple/25 to-osu-blue/35 absolute inset-0 -z-30 bg-linear-to-br"
        />
      ) : (
        <>
          <img
            data-testid="header-bg-sharp"
            src={background}
            alt=""
            draggable={false}
            className="absolute inset-0 -z-30 size-full object-cover"
          />
          {/* A blurred copy faded in by a mask: the top stays recognisable, the text area gets a calm backdrop. */}
          <img
            data-testid="header-bg-blur"
            src={background}
            alt=""
            draggable={false}
            className="absolute inset-0 -z-20 size-full scale-110 object-cover blur-md [mask-image:linear-gradient(to_bottom,transparent_20%,black_65%)]"
          />
        </>
      )}
      <div
        aria-hidden
        data-testid="header-scrim"
        className="via-background/70 to-background absolute inset-0 -z-10 bg-linear-to-b from-transparent from-15% via-55%"
      />

      <div className="flex items-start justify-between gap-2 p-2">
        {origin === undefined ? (
          <span />
        ) : (
          <p className="bg-background/75 text-foreground rounded-full px-2 py-0.5 text-[0.7rem] font-medium tracking-wide uppercase backdrop-blur-sm">
            {t(originKey(origin), { round: origin.kind === "plan" ? origin.round + 1 : 0 })}
          </p>
        )}
        <DetailsDialog window={window} background={background} details={details} />
      </div>

      <div className="flex min-w-0 flex-col gap-1 px-3 pt-12 pb-2">
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

      <div className="border-border/60 bg-background/80 flex flex-wrap items-center gap-x-2 gap-y-1 border-t px-2 py-1.5">
        <div className="flex min-w-0 flex-wrap items-center gap-1">{nav}</div>
        <Counters counters={counters} />
      </div>
    </header>
  );
}
