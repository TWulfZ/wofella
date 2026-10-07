import { Check, Undo2 } from "lucide-react";
import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { HoldButton, PatternGridPicker, patternName } from "@/features/label";
import type { PatternDefDto, SessionLabelDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { cn } from "@/shared/lib/utils";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { useSessionLabelMutations } from "../queries";

/** The answer being built: one taxonomy pattern or "no clear pattern" (ADR 0020), never a set. */
type Draft = { kind: "pattern"; id: string } | { kind: "none" } | null;

export interface SessionAnswerProps {
  keymode: number;
  md5: string;
  /** The map's newest play: its id goes with the answer as provenance. */
  playId: string | null;
  label: SessionLabelDto | null;
  title: string;
  /** The element naming the map, so each control is heard with it. */
  describedBy: string;
  taxonomy: readonly PatternDefDto[];
  holdMs: number;
  /**
   * In the Label screen the gold answer's own Save and Undo sit nearby and Enter saves gold, so the strip names its
   * controls apart and leaves focus off the grid trigger (ADR 0020).
   */
  placement: "sessionList" | "labelScreen";
  /** Trailing actions on the same line. */
  children?: ReactNode;
}

/** One writer for session answers, so the session list and the Label screen's strip cannot drift apart (ADR 0020). */
export function SessionAnswer(props: SessionAnswerProps) {
  const { keymode, md5, playId, label, title, describedBy, taxonomy, holdMs, placement, children } = props;
  const inLabelScreen = placement === "labelScreen";
  const { t } = useTranslation();
  const errorText = useErrorText();
  const [draft, setDraft] = useState<Draft>(null);
  const [error, setError] = useState<string | null>(null);
  const { submit, undo } = useSessionLabelMutations();
  const busy = submit.isPending || undo.isPending;

  const save = async (): Promise<void> => {
    if (draft === null) {
      return;
    }
    setError(null);
    try {
      await submit.mutateAsync({ keymode, md5, playId, pattern: draft.kind === "pattern" ? draft.id : null });
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

  // Untouched, the saved answer shows, so reopening the picker starts from it.
  const shownPattern = draft === null ? (label?.pattern ?? null) : draft.kind === "pattern" ? draft.id : null;
  const choiceText = shownPattern === null ? t("labelProgress.session.choose") : patternName(shownPattern);
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <PatternGridPicker
          keymode={keymode}
          taxonomy={taxonomy}
          disabled={busy || taxonomy.length === 0}
          chosen={shownPattern}
          onChoose={(pattern) => {
            setDraft({ kind: "pattern", id: pattern.id });
          }}
          title={t("labelProgress.session.pickerTitle", { title })}
          description={t("labelProgress.session.pickerDescription")}
          returnFocus={!inLabelScreen}
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
          disabled={busy}
          onClick={() => {
            setDraft((current) => (current?.kind === "none" ? null : { kind: "none" }));
          }}
          className="aria-pressed:border-primary aria-pressed:bg-primary/15 aria-pressed:text-primary"
        >
          {t("labelProgress.session.noPattern")}
        </Button>
        <HoldButton
          label={inLabelScreen ? t("labelProgress.strip.save") : t("labelProgress.session.save")}
          icon={<Check aria-hidden="true" />}
          holdMs={holdMs}
          describedBy={describedBy}
          disabled={draft === null || busy}
          onConfirm={() => {
            void save();
          }}
        />
        {label !== null && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={busy}
            aria-describedby={describedBy}
            onClick={() => {
              void runUndo(label.eventId);
            }}
          >
            <Undo2 aria-hidden="true" />
            {inLabelScreen ? t("labelProgress.strip.undo") : t("labelProgress.session.undo")}
          </Button>
        )}
        {children}
      </div>
      {error !== null && (
        <p role="alert" className="text-destructive text-sm">
          {error}
        </p>
      )}
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
