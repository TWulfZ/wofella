import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ExternalLink, Trophy } from "lucide-react";
import { type RefCallback, useId } from "react";
import { Trans, useTranslation } from "react-i18next";
import { backgroundDataUrl, chartBackgroundQuery, StarRating } from "@/features/label";
import type { PatternDefDto, SessionPlayDto, SessionPlaysDto } from "@/ipc/bindings";
import { formatDateTime, formatRelative } from "@/shared/format";
import { cn } from "@/shared/lib/utils";
import { Badge } from "@/shared/ui/badge";
import { buttonVariants } from "@/shared/ui/button";
import { sessionCountsText } from "../countText";
import { type SessionMap, sessionMapCounts, sessionMaps } from "../model";
import { useNearViewport } from "../useNearViewport";
import { QueryAlert } from "./QueryAlert";
import { SessionStatusPill } from "./SessionAnswer";
import { SessionListAnswer } from "./SessionListAnswer";

export interface SessionThumbnailParams {
  /** How far outside the viewport a row starts loading its image, so it is there by the time it scrolls in. */
  rootMargin: string;
}

interface SessionRowProps {
  map: SessionMap;
  keymode: number;
  taxonomy: readonly PatternDefDto[];
  nowMs: number;
  thumbnail: SessionThumbnailParams;
}

function useLazyCover(md5: string, params: SessionThumbnailParams): [RefCallback<HTMLDivElement>, string | null] {
  const [ref, near] = useNearViewport<HTMLDivElement>(params.rootMargin);
  const background = useQuery(chartBackgroundQuery(near ? md5 : null));
  return [ref, backgroundDataUrl(background.data)];
}

function MapCover({ coverRef, src }: { coverRef: RefCallback<HTMLDivElement>; src: string | null }) {
  // Fixed square, so the image arriving or failing moves nothing.
  return (
    <div
      ref={coverRef}
      aria-hidden
      data-testid="session-thumb"
      className="ring-border relative size-24 shrink-0 self-start overflow-hidden rounded-lg shadow-md shadow-black/40 ring-1"
    >
      {src === null ? (
        <div data-testid="session-thumb-fallback" className="from-osu-pink/35 via-osu-purple/25 to-osu-blue/35 size-full bg-linear-to-br" />
      ) : (
        <img
          data-testid="session-thumb-image"
          src={src}
          alt=""
          draggable={false}
          decoding="async"
          className="size-full object-cover motion-safe:animate-in motion-safe:fade-in-0 motion-safe:duration-200"
        />
      )}
    </div>
  );
}

function SessionRow({ map, keymode, taxonomy, nowMs, thumbnail }: SessionRowProps) {
  const play: SessionPlayDto = map.newest;
  const { t, i18n } = useTranslation();
  const titleId = useId();
  const [coverRef, cover] = useLazyCover(play.md5, thumbnail);
  return (
    <li aria-labelledby={titleId} className="bg-card ring-border @container relative isolate overflow-hidden rounded-xl ring-1">
      {/* osu!web's beatmap card: the cover faded behind the text. Even over a white cover the scrim's thinnest edge
          (card at 75 % over the image at 40 %) keeps muted text at about 5:1. */}
      <div aria-hidden="true" className="absolute inset-0 -z-10">
        {cover !== null && (
          <img
            data-testid="session-card-backdrop"
            src={cover}
            alt=""
            draggable={false}
            decoding="async"
            className="size-full scale-110 object-cover opacity-40 blur-sm motion-safe:animate-in motion-safe:fade-in-0 motion-safe:duration-300"
          />
        )}
        <div
          data-testid="session-card-scrim"
          className="from-card via-card/90 to-card/75 absolute inset-0 bg-linear-to-r"
        />
      </div>
      {/* Below a 42rem card the answer wraps under the map instead of squeezing the title. */}
      <div className="flex flex-wrap gap-3 p-3 @2xl:flex-nowrap">
        <MapCover coverRef={coverRef} src={cover} />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <div className="flex min-w-0 items-start gap-2">
            <div className="flex min-w-0 flex-1 flex-col">
              <span id={titleId} className="font-display min-w-0 truncate text-base leading-tight font-bold">
                {play.title}
              </span>
              <span className="text-foreground/90 min-w-0 truncate text-sm">
                {t("labelProgress.session.byArtist", { artist: play.artist })}
              </span>
              <span className="text-muted-foreground min-w-0 truncate text-xs">
                <Trans
                  i18nKey="labelProgress.session.mappedBy"
                  values={{ creator: play.creator }}
                  components={{ creator: <span className="text-osu-pink font-semibold" /> }}
                />
              </span>
            </div>
            {/* Once labelled, the answer tile shows the answer; a pill would only repeat it. */}
            {play.label === null && <SessionStatusPill label={null} className="shrink-0" />}
          </div>
          <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
            {play.stars !== null && <StarRating stars={play.stars} />}
            <span className="min-w-0 max-w-56 truncate text-sm font-semibold">{play.version}</span>
            {play.goldWindows > 0 && (
              <Badge variant="outline" className="border-osu-yellow/40 bg-background/60 text-foreground">
                <Trophy aria-hidden="true" className="text-osu-yellow" />
                {t("labelProgress.session.goldWindows", { count: play.goldWindows })}
              </Badge>
            )}
            {map.playCount > 1 && (
              <span className="text-muted-foreground text-xs font-semibold tabular-nums">
                <span aria-hidden="true">{t("labelProgress.session.playCount", { count: map.playCount })}</span>
                <span className="sr-only">{t("labelProgress.session.timesPlayed", { count: map.playCount })}</span>
              </span>
            )}
            <time dateTime={play.playedAt} title={formatDateTime(play.playedAt, i18n.language)} className="text-muted-foreground text-xs">
              {t("labelProgress.session.playedAt", { when: formatRelative(play.playedAt, nowMs, i18n.language) })}
            </time>
          </div>
        </div>
        <div className="w-full @2xl:w-64 @2xl:shrink-0">
          <SessionListAnswer
            keymode={keymode}
            md5={play.md5}
            playId={play.playId}
            label={play.label}
            title={play.title}
            describedBy={titleId}
            taxonomy={taxonomy}
          >
            <Link
              to="/label"
              search={(prev) => ({ ...prev, chart: play.md5 })}
              aria-describedby={titleId}
              className={buttonVariants({ variant: "ghost", size: "sm" })}
            >
              <ExternalLink aria-hidden="true" />
              {t("labelProgress.session.open")}
            </Link>
          </SessionListAnswer>
        </div>
      </div>
    </li>
  );
}

interface MapGroupProps extends Omit<SessionRowProps, "map"> {
  heading: string;
  maps: readonly SessionMap[];
  secondary?: boolean;
}

function MapGroup({ heading, maps, secondary = false, ...row }: MapGroupProps) {
  const headingId = useId();
  return (
    <div className="flex flex-col gap-2">
      <h3
        id={headingId}
        className={cn("text-xs font-semibold tracking-wide uppercase", secondary ? "text-muted-foreground" : "text-osu-yellow")}
      >
        {heading}
      </h3>
      <ul aria-labelledby={headingId} className="flex flex-col gap-2">
        {maps.map((map) => (
          <SessionRow key={map.newest.md5} map={map} {...row} />
        ))}
      </ul>
    </div>
  );
}

interface SessionListProps {
  keymode: number;
  session: UseQueryResult<SessionPlaysDto>;
  taxonomy: readonly PatternDefDto[];
  /** Set when the taxonomy failed: without it no answer can be picked. */
  taxonomyFailure?: { error: unknown; retry: () => void } | undefined;
  nowMs: number;
  thumbnail: SessionThumbnailParams;
}

export function SessionList({ keymode, session, taxonomy, taxonomyFailure, nowMs, thumbnail }: SessionListProps) {
  const { t } = useTranslation();
  const headingId = useId();
  const plays = session.data?.plays;
  const counts = plays === undefined ? undefined : sessionMapCounts(plays);
  const maps = plays === undefined ? undefined : sessionMaps(plays);
  const row = { keymode, taxonomy, nowMs, thumbnail };
  return (
    <section aria-labelledby={headingId} className="bg-card ring-border flex flex-col gap-4 rounded-xl p-5 shadow-lg shadow-black/20 ring-1">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-1">
          <h2 id={headingId} className="font-display text-base font-semibold">
            {t("labelProgress.session.title")}
          </h2>
          <p className="text-muted-foreground max-w-2xl text-sm">{t("labelProgress.session.description")}</p>
        </div>
        {counts !== undefined && plays !== undefined && plays.length > 0 && (
          <p className="text-muted-foreground text-sm tabular-nums">{sessionCountsText(t, "labelProgress.session.counts", counts)}</p>
        )}
      </div>
      {taxonomyFailure !== undefined && (
        <QueryAlert error={taxonomyFailure.error} onRetry={taxonomyFailure.retry} lead={t("labelProgress.session.noTaxonomy")} />
      )}
      {session.isError ? (
        <QueryAlert
          error={session.error}
          onRetry={() => {
            void session.refetch();
          }}
        />
      ) : maps === undefined ? (
        <p className="text-muted-foreground text-sm">{t("common.loading")}</p>
      ) : maps.pending.length + maps.labelled.length === 0 ? (
        <p className="bg-surface-raised text-muted-foreground rounded-lg px-4 py-6 text-center text-sm">
          {t("labelProgress.session.empty")}
        </p>
      ) : (
        <div className="flex flex-col gap-5">
          {maps.pending.length > 0 && <MapGroup heading={t("labelProgress.session.toLabel")} maps={maps.pending} {...row} />}
          {maps.labelled.length > 0 && (
            <MapGroup heading={t("labelProgress.session.labelledGroup")} maps={maps.labelled} secondary {...row} />
          )}
        </div>
      )}
    </section>
  );
}
