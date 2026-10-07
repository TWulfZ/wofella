import type { UseQueryResult } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Check, ExternalLink, Star, Undo2 } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { HoldButton, PatternPicker, patternName } from "@/features/label";
import type { PatternDefDto, SessionPlayDto, SessionPlaysDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { formatDateTime, formatRelative } from "@/shared/format";
import { cn } from "@/shared/lib/utils";
import { Badge } from "@/shared/ui/badge";
import { Button, buttonVariants } from "@/shared/ui/button";
import { sessionCountsText } from "../countText";
import { type SessionMap, sessionMapCounts, sessionMaps } from "../model";
import { useSessionLabelMutations } from "../queries";
import { QueryAlert } from "./QueryAlert";

/** The answer being built: one taxonomy pattern or "no clear pattern" (ADR 0020), never a set. */
type Draft = { kind: "pattern"; id: string } | { kind: "none" } | null;

interface SessionRowProps {
  map: SessionMap;
  keymode: number;
  taxonomy: readonly PatternDefDto[];
  holdMs: number;
  nowMs: number;
}

function answerText(pattern: string | null, noPatternText: string): string {
  return pattern === null ? noPatternText : patternName(pattern);
}

function SessionRow({ map, keymode, taxonomy, holdMs, nowMs }: SessionRowProps) {
  const play: SessionPlayDto = map.newest;
  const { t, i18n } = useTranslation();
  const errorText = useErrorText();
  const titleId = useId();
  const [draft, setDraft] = useState<Draft>(null);
  const [error, setError] = useState<string | null>(null);
  const { submit, undo } = useSessionLabelMutations();
  const busy = submit.isPending || undo.isPending;
  const noPatternText = t("labelProgress.session.noPattern");

  const save = async (): Promise<void> => {
    if (draft === null) {
      return;
    }
    setError(null);
    try {
      await submit.mutateAsync({
        keymode,
        md5: play.md5,
        playId: play.playId,
        pattern: draft.kind === "pattern" ? draft.id : null,
      });
      setDraft(null);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const runUndo = async (eventId: string): Promise<void> => {
    setError(null);
    try {
      await undo.mutateAsync(eventId);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const shownPattern = draft?.kind === "pattern" ? draft.id : null;
  const choiceText = shownPattern === null ? t("labelProgress.session.choose") : patternName(shownPattern);
  return (
    <li aria-labelledby={titleId} className="flex flex-col gap-2 py-3 first:pt-0 last:pb-0">
      <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
        <span id={titleId} className="min-w-0 truncate font-medium">
          {play.title}
        </span>
        <Badge variant="secondary" className="max-w-48 truncate">
          {play.version}
        </Badge>
        {play.stars !== null && (
          <span
            aria-label={t("labelProgress.session.stars", { stars: play.stars.toFixed(2) })}
            role="img"
            className="text-osu-yellow inline-flex items-center gap-1 text-xs font-semibold tabular-nums"
          >
            <Star aria-hidden="true" className="size-3 fill-current" />
            <span aria-hidden="true">{play.stars.toFixed(2)}</span>
          </span>
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
        <span className="ml-auto">
          {play.label === null ? (
            <Badge variant="outline" className="border-osu-yellow/40 text-osu-yellow">
              {t("labelProgress.session.pending")}
            </Badge>
          ) : (
            <Badge variant="default" className="capitalize">
              <Check aria-hidden="true" />
              {answerText(play.label.pattern, noPatternText)}
            </Badge>
          )}
        </span>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <PatternPicker
          taxonomy={taxonomy}
          single
          side="bottom"
          disabled={busy || taxonomy.length === 0}
          isChosen={(pattern) => pattern.id === shownPattern}
          onPick={(pattern) => {
            setDraft({ kind: "pattern", id: pattern.id });
          }}
          trigger={{
            // The visible choice is part of the name (WCAG 2.5.3), so it is heard and can be spoken to.
            label: t("labelProgress.session.chooseFor", { title: play.title, choice: choiceText }),
            text: shownPattern === null ? choiceText : <span className="capitalize">{choiceText}</span>,
          }}
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-pressed={draft?.kind === "none"}
          aria-describedby={titleId}
          disabled={busy}
          onClick={() => {
            setDraft((current) => (current?.kind === "none" ? null : { kind: "none" }));
          }}
          className="aria-pressed:border-primary aria-pressed:bg-primary/15 aria-pressed:text-primary"
        >
          {noPatternText}
        </Button>
        <HoldButton
          label={t("labelProgress.session.save")}
          icon={<Check aria-hidden="true" />}
          holdMs={holdMs}
          describedBy={titleId}
          disabled={draft === null || busy}
          onConfirm={() => {
            void save();
          }}
        />
        {play.label !== null && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={busy}
            aria-describedby={titleId}
            onClick={() => {
              if (play.label !== null) {
                void runUndo(play.label.eventId);
              }
            }}
          >
            <Undo2 aria-hidden="true" />
            {t("labelProgress.session.undo")}
          </Button>
        )}
        <Link
          to="/label"
          search={(prev) => ({ ...prev, chart: play.md5 })}
          aria-describedby={titleId}
          className={cn(buttonVariants({ variant: "ghost", size: "sm" }), "ml-auto")}
        >
          <ExternalLink aria-hidden="true" />
          {t("labelProgress.session.open")}
        </Link>
      </div>
      {error !== null && (
        <p role="alert" className="text-destructive text-sm">
          {error}
        </p>
      )}
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
      <ul aria-labelledby={headingId} className="flex flex-col divide-y">
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
  holdMs: number;
  nowMs: number;
}

export function SessionList({ keymode, session, taxonomy, taxonomyFailure, holdMs, nowMs }: SessionListProps) {
  const { t } = useTranslation();
  const headingId = useId();
  const plays = session.data?.plays;
  const counts = plays === undefined ? undefined : sessionMapCounts(plays);
  const maps = plays === undefined ? undefined : sessionMaps(plays);
  const row = { keymode, taxonomy, holdMs, nowMs };
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
