import { Check, Copy, ExternalLink, Maximize2, SkipForward, Star, Trophy, Undo2 } from "lucide-react";
import { type ComponentType, type ReactNode, type RefObject, type SVGProps, useEffect, useEffectEvent, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@/ipc/opener";
import { cn } from "@/shared/lib/utils";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/shared/ui/dialog";
import { formatMinSec } from "../format";
import type { WindowOrigin } from "../session";
import { difficultyColour, starTextColour } from "../starColour";
import type { LabelWindow } from "../types";
import { BpmIcon, LengthIcon, LongNoteCountIcon, NoteCountIcon } from "./statIcons";

export const CHART_HEADER_PARAMS = {
  /** Height of the sticky title bar the card collapses into; the panel keeps focused cards clear of it. */
  compactBarPx: 44,
  /** How long the "copied" confirmation stays before the status line clears. */
  copiedFeedbackMs: 2000,
} as const;

const OSU_WEB = "https://osu.ppy.sh";

/** osu!web's page for the difficulty, or for the whole set when the difficulty has no ID; null for unsubmitted maps. */
export function osuWebUrl(setId: number | null, beatmapId: number | null): string | null {
  if (setId === null) {
    return null;
  }
  const set = `${OSU_WEB}/beatmapsets/${String(setId)}`;
  return beatmapId === null ? set : `${set}#mania/${String(beatmapId)}`;
}

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
  /** The panel's scroll container: the card collapses into a sticky bar once its title scrolls out of it. */
  scrollRoot?: RefObject<HTMLElement | null> | undefined;
  /** The expanded card's height, for a backdrop the panel paints under its scrollbar. */
  onBlockSize?: ((px: number) => void) | undefined;
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

function DetailRow({ term, children, wide = false }: { term: string; children: ReactNode; wide?: boolean }) {
  return (
    <div className={cn("flex min-w-0 flex-col gap-0.5", wide && "sm:col-span-2")}>
      <dt className="text-muted-foreground text-xs">{term}</dt>
      <dd className="min-w-0 break-words">{children}</dd>
    </div>
  );
}

interface StatProps {
  icon: ComponentType<SVGProps<SVGSVGElement>>;
  caption: string;
  value: string;
}

function Stat({ icon: Icon, caption, value }: StatProps) {
  return (
    <li className="bg-background/80 flex min-w-0 items-center gap-2 rounded-lg border px-3 py-2">
      <Icon className="text-osu-yellow size-5 shrink-0" />
      <span className="flex min-w-0 flex-col leading-tight">
        <span className="font-semibold tabular-nums">{value}</span>
        <span data-stat-caption className="text-muted-foreground text-xs">
          {caption}
        </span>
      </span>
    </li>
  );
}

function StatRow({ details }: { details: ChartDetails }) {
  const { t, i18n } = useTranslation();
  const count = new Intl.NumberFormat(i18n.language);
  const bpm = bpmText(details.bpmMin, details.bpmMax);
  return (
    <ul aria-label={t("label.header.stats")} className="grid grid-cols-2 gap-2 sm:grid-cols-4">
      <Stat icon={LengthIcon} caption={t("label.header.field.length")} value={formatMinSec(details.lengthMs)} />
      {bpm !== null && <Stat icon={BpmIcon} caption={t("label.header.field.bpm")} value={bpm} />}
      <Stat icon={NoteCountIcon} caption={t("label.header.field.notes")} value={count.format(details.nNotes)} />
      <Stat icon={LongNoteCountIcon} caption={t("label.header.field.longNotes")} value={count.format(details.nLn)} />
    </ul>
  );
}

function Md5({ md5 }: { md5: string }) {
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
      await navigator.clipboard.writeText(md5);
    } catch {
      next = "copyFailed";
    }
    setStatus(next);
    if (timer.current !== null) {
      clearTimeout(timer.current);
    }
    timer.current = setTimeout(() => {
      setStatus(null);
    }, CHART_HEADER_PARAMS.copiedFeedbackMs);
  };
  const label = t("label.header.copyMd5");
  return (
    <span className="flex min-w-0 items-center gap-1.5">
      <span className="font-mono text-xs break-all">{md5}</span>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={label}
        title={label}
        onClick={() => {
          void copy();
        }}
        className="shrink-0"
      >
        <Copy aria-hidden />
      </Button>
      <span role="status" className="text-muted-foreground text-xs">
        {status === null ? "" : t(`label.header.${status}`)}
      </span>
    </span>
  );
}

function OpenOnOsu({ url }: { url: string }) {
  const { t } = useTranslation();
  const [failed, setFailed] = useState(false);
  const open = async (): Promise<void> => {
    setFailed(false);
    try {
      await openUrl(url);
    } catch {
      setFailed(true);
    }
  };
  return (
    <div className="flex flex-wrap items-center gap-2">
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => {
          void open();
        }}
      >
        <ExternalLink aria-hidden />
        {t("label.header.openOnOsu")}
      </Button>
      {failed && (
        <p role="alert" className="text-destructive text-xs">
          {t("label.header.openFailed")}
        </p>
      )}
    </div>
  );
}

function DetailsDialog({ window, background, details }: Pick<ChartHeaderProps, "window" | "background" | "details">) {
  const { t } = useTranslation();
  const title = details?.title ?? window.title;
  const stars = details?.stars ?? window.stars;
  const source = details?.source?.trim() ?? "";
  const url = details === undefined ? null : osuWebUrl(details.setId, details.beatmapId);
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
          className="bg-background/70 hover:bg-background/90 shrink-0 backdrop-blur-sm"
        >
          <Maximize2 aria-hidden />
        </Button>
      </DialogTrigger>
      <DialogContent closeLabel={t("common.close")} className="max-h-[92dvh] gap-5 overflow-y-auto sm:max-w-3xl">
        <DialogHeader className="pr-8">
          <DialogTitle className="font-display text-xl leading-tight font-semibold">{title}</DialogTitle>
          <DialogDescription>{details?.artist ?? window.artist}</DialogDescription>
        </DialogHeader>
        {background != null && (
          <img
            src={background}
            alt={t("label.header.imageAlt", { title })}
            draggable={false}
            className="bg-background max-h-[50dvh] w-full rounded-lg object-contain"
          />
        )}
        {details !== undefined && <StatRow details={details} />}
        {url !== null && <OpenOnOsu url={url} />}
        <dl className="grid grid-cols-1 gap-x-6 gap-y-3 text-sm sm:grid-cols-2">
          <DetailRow term={t("label.header.field.mapper")}>{details?.creator ?? window.creator}</DetailRow>
          <DetailRow term={t("label.header.field.difficulty")}>{details?.version ?? window.version}</DetailRow>
          {stars !== null && (
            <DetailRow term={t("label.header.field.stars")}>
              <StarRating stars={stars} />
            </DetailRow>
          )}
          {details !== undefined && (
            <>
              {details.od !== null && <DetailRow term={t("label.header.field.od")}>{decimal(details.od)}</DetailRow>}
              {details.hp !== null && <DetailRow term={t("label.header.field.hp")}>{decimal(details.hp)}</DetailRow>}
              {source !== "" && <DetailRow term={t("label.header.field.source")}>{source}</DetailRow>}
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
              {details.tags.length > 0 && (
                <DetailRow term={t("label.header.field.tags")} wide>
                  <ul className="flex flex-wrap gap-1.5">
                    {/* Mappers repeat words in the tag string; the tag doubles as the key, so it must be unique. */}
                    {[...new Set(details.tags)].map((tag) => (
                      <li key={tag} className="bg-muted rounded-full px-2.5 text-xs leading-6">
                        {tag}
                      </li>
                    ))}
                  </ul>
                </DetailRow>
              )}
            </>
          )}
          <DetailRow term={t("label.header.field.md5")} wide>
            <Md5 md5={details?.md5 ?? window.anchor.md5} />
          </DetailRow>
        </dl>
      </DialogContent>
    </Dialog>
  );
}

function OriginBadge({ origin }: { origin: WindowOrigin }) {
  const { t } = useTranslation();
  return (
    <p className="bg-background/75 text-foreground shrink-0 rounded-full px-2 py-0.5 text-[0.7rem] font-medium tracking-wide uppercase backdrop-blur-sm">
      {t(originKey(origin), { round: origin.kind === "plan" ? origin.round + 1 : 0 })}
    </p>
  );
}

/** True once `target` has scrolled above the root's top edge, less the compact bar that would cover it. */
function useScrolledAway(target: RefObject<HTMLElement | null>, root: RefObject<HTMLElement | null> | undefined): boolean {
  const [away, setAway] = useState(false);
  useEffect(() => {
    const el = target.current;
    // Without the observer the full card simply stays: the collapse only saves room.
    if (el === null || typeof IntersectionObserver === "undefined") {
      return;
    }
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry !== undefined) {
          setAway(!entry.isIntersecting);
        }
      },
      { root: root?.current ?? null, rootMargin: `-${String(CHART_HEADER_PARAMS.compactBarPx)}px 0px 0px 0px` },
    );
    observer.observe(el);
    return () => {
      observer.disconnect();
    };
  }, [target, root]);
  return away;
}

function useBlockSize(onBlockSize: ((px: number) => void) | undefined) {
  const ref = useRef<HTMLElement>(null);
  const report = useEffectEvent((px: number) => {
    onBlockSize?.(px);
  });
  useEffect(() => {
    const el = ref.current;
    if (el === null || typeof ResizeObserver === "undefined") {
      return;
    }
    const observer = new ResizeObserver(([entry]) => {
      const box = entry?.borderBoxSize[0];
      if (box !== undefined) {
        report(box.blockSize);
      }
    });
    observer.observe(el);
    return () => {
      observer.disconnect();
    };
  }, []);
  return ref;
}

interface CompactBarProps {
  window: LabelWindow;
  origin: WindowOrigin | undefined;
  background: string | null;
  details: ChartDetails | undefined;
  nav: ReactNode;
}

function CompactBar({ window, origin, background, details, nav }: CompactBarProps) {
  const { t } = useTranslation();
  return (
    <div
      role="group"
      aria-label={t("label.header.compact")}
      data-testid="header-compact"
      style={{ height: CHART_HEADER_PARAMS.compactBarPx }}
      className={cn(
        "bg-surface-raised/95 absolute inset-x-0 top-0 flex items-center gap-2 overflow-hidden border-b pr-2 shadow-md backdrop-blur",
        "motion-safe:animate-in motion-safe:fade-in-0 motion-safe:slide-in-from-top-2 motion-safe:duration-150",
      )}
    >
      {background === null ? (
        <div aria-hidden className="from-osu-pink/35 via-osu-purple/25 to-osu-blue/35 h-full w-16 shrink-0 bg-linear-to-br" />
      ) : (
        <img
          data-testid="header-compact-thumb"
          src={background}
          alt=""
          draggable={false}
          className="h-full w-16 shrink-0 object-cover [mask-image:linear-gradient(to_right,black_55%,transparent)]"
        />
      )}
      <p title={window.title} className="font-display min-w-0 flex-1 truncate text-sm font-semibold">
        {window.title}
      </p>
      {window.stars !== null && <StarRating stars={window.stars} />}
      {origin !== undefined && <OriginBadge origin={origin} />}
      <div className="shrink-0">{nav}</div>
      <DetailsDialog window={window} background={background} details={details} />
    </div>
  );
}

export function ChartHeader(props: ChartHeaderProps) {
  const { window, origin, background = null, details, nav, counters, scrollRoot, onBlockSize } = props;
  const { t } = useTranslation();
  const sentinel = useRef<HTMLDivElement>(null);
  const collapsed = useScrolledAway(sentinel, scrollRoot);
  const headerRef = useBlockSize(onBlockSize);
  return (
    <>
      {/* Zero height, so it takes no room while the card shows; it only anchors the bar to the panel's top edge. */}
      <div className="sticky top-0 z-20 h-0">
        {collapsed && <CompactBar window={window} origin={origin} background={background} details={details} nav={nav} />}
      </div>
      {/* Full-bleed: the image is the panel's top background, so the card adds no frame or inset of its own. */}
      <header ref={headerRef} className="relative isolate flex w-full shrink-0 flex-col overflow-hidden rounded-t-xl border-b">
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

        {/* Inert while the bar carries these, so each control exists once for the keyboard and assistive tech. */}
        <div inert={collapsed} className="flex items-start justify-between gap-2 p-2">
          {origin === undefined ? <span /> : <OriginBadge origin={origin} />}
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
        <div
          inert={collapsed}
          className="border-border/60 bg-background/80 flex flex-wrap items-center gap-x-2 gap-y-1 border-t px-2 py-1.5"
        >
          <div className="flex min-w-0 flex-wrap items-center gap-1">{nav}</div>
          <Counters counters={counters} />
        </div>
        {/* The card's lower edge, below the navigation: the bar takes over only once no card control is still on screen. */}
        <div ref={sentinel} aria-hidden data-testid="header-sentinel" className="h-px w-full" />
      </header>
    </>
  );
}
