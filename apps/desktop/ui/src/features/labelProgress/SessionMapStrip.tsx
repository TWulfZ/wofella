import { useQuery } from "@tanstack/react-query";
import { Gamepad2 } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { HOLD_BUTTON_PARAMS, labelTaxonomyQuery } from "@/features/label";
import type { PatternDefDto } from "@/ipc/bindings";
import { QueryAlert } from "./components/QueryAlert";
import { SessionStatusPill, SessionStripAnswer } from "./components/SessionAnswer";
import { newestPlayOf } from "./model";
import { sessionPlaysQuery } from "./queries";

export interface SessionMapStripProps {
  keymode: number;
  md5: string;
  title: string;
  holdMs?: number;
}

const NO_TAXONOMY: readonly PatternDefDto[] = [];

/**
 * The session list's dominant-pattern answer for the map the Label screen shows. Saving resolves the map's row; the gold
 * windows saved around it never do, since they are different evidence (ADR 0020).
 */
export function SessionMapStrip({ keymode, md5, title, holdMs = HOLD_BUTTON_PARAMS.defaultHoldMs }: SessionMapStripProps) {
  const { t } = useTranslation();
  const headingId = useId();
  const titleId = useId();
  const session = useQuery(sessionPlaysQuery(keymode));
  const taxonomy = useQuery(labelTaxonomyQuery(keymode));
  const play = session.data === undefined ? undefined : newestPlayOf(session.data.plays, md5);
  return (
    <section
      aria-labelledby={headingId}
      className="bg-card ring-osu-blue/30 flex flex-col gap-2 rounded-xl p-3 shadow-lg shadow-black/20 ring-1"
    >
      <div className="flex flex-wrap items-center gap-2">
        <Gamepad2 aria-hidden="true" className="text-osu-blue size-4 shrink-0" />
        <h3 id={headingId} className="min-w-0 flex-1 text-sm font-semibold">
          {t("labelProgress.strip.title")}
        </h3>
        {play !== undefined && play !== null && <SessionStatusPill label={play.label} />}
      </div>
      <span id={titleId} className="sr-only">
        {title}
      </span>
      <p className="text-muted-foreground text-xs">{t("labelProgress.strip.description")}</p>
      {session.isError ? (
        <QueryAlert
          error={session.error}
          onRetry={() => {
            void session.refetch();
          }}
        />
      ) : taxonomy.isError ? (
        <QueryAlert
          error={taxonomy.error}
          onRetry={() => {
            void taxonomy.refetch();
          }}
          lead={t("labelProgress.session.noTaxonomy")}
        />
      ) : play === undefined ? (
        <p className="text-muted-foreground text-sm">{t("common.loading")}</p>
      ) : play === null ? (
        <p className="text-muted-foreground text-sm">{t("labelProgress.strip.notInSession")}</p>
      ) : (
        <SessionStripAnswer
          key={md5}
          keymode={keymode}
          md5={md5}
          playId={play.playId}
          label={play.label}
          title={title}
          describedBy={titleId}
          taxonomy={taxonomy.data ?? NO_TAXONOMY}
          holdMs={holdMs}
        />
      )}
    </section>
  );
}
