import { Check, Undo2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { HoldButton, PatternGridPicker, patternName } from "@/features/label";
import type { PatternDefDto, SessionLabelDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { cn } from "@/shared/lib/utils";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { useSessionLabelMutations } from "../queries";

/** One taxonomy pattern, or null for "no clear pattern" (ADR 0020), never a set. */
export interface SessionAnswerChoice {
  pattern: string | null;
}

export interface SessionAnswerTarget {
  keymode: number;
  md5: string;
  /** The map's newest play: its id goes with the answer as provenance. */
  playId: string | null;
}

/** One writer for session answers, so the session list and the Label screen's strip cannot drift apart (ADR 0020). */
export function useSessionAnswerWriter({ keymode, md5, playId }: SessionAnswerTarget) {
  const errorText = useErrorText();
  const [error, setError] = useState<string | null>(null);
  const { submit, undo } = useSessionLabelMutations();

  const run = async (action: () => Promise<unknown>): Promise<boolean> => {
    setError(null);
    try {
      await action();
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  };

  return {
    busy: submit.isPending || undo.isPending,
    error,
    save: (pattern: string | null) => run(() => submit.mutateAsync({ keymode, md5, playId, pattern })),
    undo: (eventId: string) => run(() => undo.mutateAsync(eventId)),
  };
}

export function SessionAnswerError({ error }: { error: string | null }) {
  return error === null ? null : (
    <p role="alert" className="text-destructive text-sm">
      {error}
    </p>
  );
}

/** The answer being built in the strip; null until something is picked. */
type Draft = { kind: "pattern"; id: string } | { kind: "none" } | null;

export interface SessionStripAnswerProps extends SessionAnswerTarget {
  label: SessionLabelDto | null;
  title: string;
  /** The element naming the map, so each control is heard with it. */
  describedBy: string;
  taxonomy: readonly PatternDefDto[];
  holdMs: number;
}

/**
 * The Label screen's answer: a draft saved by a hold, because Enter saves gold there. The gold answer's own Save and
 * Undo sit nearby, so the strip names its controls apart and leaves focus off the grid trigger (ADR 0020).
 */
export function SessionStripAnswer(props: SessionStripAnswerProps) {
  const { label, title, describedBy, taxonomy, holdMs } = props;
  const { t } = useTranslation();
  const [draft, setDraft] = useState<Draft>(null);
  const writer = useSessionAnswerWriter(props);

  const save = async (): Promise<void> => {
    if (draft !== null && (await writer.save(draft.kind === "pattern" ? draft.id : null))) {
      setDraft(null);
    }
  };

  // Untouched, the saved answer shows, so reopening the picker starts from it.
  const shownPattern = draft === null ? (label?.pattern ?? null) : draft.kind === "pattern" ? draft.id : null;
  const choiceText = shownPattern === null ? t("labelProgress.session.choose") : patternName(shownPattern);
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <PatternGridPicker
          keymode={props.keymode}
          taxonomy={taxonomy}
          disabled={writer.busy || taxonomy.length === 0}
          chosen={shownPattern}
          onChoose={(pattern) => {
            setDraft({ kind: "pattern", id: pattern.id });
          }}
          title={t("labelProgress.session.pickerTitle", { title })}
          description={t("labelProgress.session.pickerDescription")}
          returnFocus={false}
          trigger={{
            // The visible choice is part of the name (WCAG 2.5.3), so it is heard and can be spoken to.
            label: t("labelProgress.session.chooseFor", { title, choice: choiceText }),
            text: shownPattern === null ? choiceText : <span className="capitalize">{choiceText}</span>,
          }}
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-pressed={draft?.kind === "none"}
          aria-describedby={describedBy}
          disabled={writer.busy}
          onClick={() => {
            setDraft((current) => (current?.kind === "none" ? null : { kind: "none" }));
          }}
          className="aria-pressed:border-primary aria-pressed:bg-primary/15 aria-pressed:text-primary"
        >
          {t("labelProgress.session.noPattern")}
        </Button>
        <HoldButton
          label={t("labelProgress.strip.save")}
          icon={<Check aria-hidden="true" />}
          holdMs={holdMs}
          describedBy={describedBy}
          disabled={draft === null || writer.busy}
          onConfirm={() => {
            void save();
          }}
        />
        {label !== null && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={writer.busy}
            aria-describedby={describedBy}
            onClick={() => {
              void writer.undo(label.eventId);
            }}
          >
            <Undo2 aria-hidden="true" />
            {t("labelProgress.strip.undo")}
          </Button>
        )}
      </div>
      <SessionAnswerError error={writer.error} />
    </div>
  );
}

/** Where osu!web shows RANKED: whether the map still waits for its answer, or the answer. */
export function SessionStatusPill({ label, className }: { label: SessionLabelDto | null; className?: string }) {
  const { t } = useTranslation();
  return label === null ? (
    <Badge
      data-testid="session-status"
      variant="outline"
      className={cn("border-osu-yellow/50 bg-background/70 text-osu-yellow font-bold uppercase", className)}
    >
      {t("labelProgress.session.pending")}
    </Badge>
  ) : (
    <Badge data-testid="session-status" variant="default" className={cn("max-w-56 font-bold", className)}>
      <Check aria-hidden="true" />
      <span className="truncate capitalize">
        {label.pattern === null ? t("labelProgress.session.noPattern") : patternName(label.pattern)}
      </span>
    </Badge>
  );
}
