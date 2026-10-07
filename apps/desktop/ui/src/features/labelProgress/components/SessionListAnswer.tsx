import { LayoutGrid, Undo2 } from "lucide-react";
import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { AxisIcon, axisFamily, axisKey, familyAccent, PatternGridPicker, patternName } from "@/features/label";
import type { PatternDefDto, SessionLabelDto } from "@/ipc/bindings";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { type SessionAnswerChoice, SessionAnswerError, type SessionAnswerTarget, useSessionAnswerWriter } from "./SessionAnswer";

export interface SessionListAnswerProps extends SessionAnswerTarget {
  label: SessionLabelDto | null;
  title: string;
  /** The element naming the map, so each control is heard with it. */
  describedBy: string;
  taxonomy: readonly PatternDefDto[];
  /** Trailing actions under the tile. */
  children?: ReactNode;
}

const TILE_BASE = cn(
  "relative flex min-h-16 w-full cursor-pointer items-center gap-3 overflow-hidden rounded-xl border py-2 pr-3 text-left outline-none",
  "motion-safe:transition-[background-color,border-color,color] motion-safe:duration-150 motion-safe:ease-out",
  "focus-visible:border-ring focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-2",
  "disabled:cursor-not-allowed disabled:opacity-60",
);

interface Tile {
  className: string;
  content: ReactNode;
}

function useTile(choice: SessionAnswerChoice | null, taxonomy: readonly PatternDefDto[]): Tile {
  const { t } = useTranslation();
  if (choice === null) {
    return {
      className: cn(
        TILE_BASE,
        "border-control-border bg-background/60 text-muted-foreground border-dashed pl-4",
        "enabled:hover:border-primary enabled:hover:bg-primary/10 enabled:hover:text-primary",
      ),
      content: (
        <>
          <LayoutGrid aria-hidden="true" className="size-6 shrink-0" />
          <span className="font-semibold">{t("labelProgress.session.choose")}</span>
        </>
      ),
    };
  }
  if (choice.pattern === null) {
    return {
      className: cn(TILE_BASE, "border-border bg-muted/60 text-muted-foreground enabled:hover:bg-surface-raised pl-4"),
      content: (
        <>
          <AxisIcon axis="" className="size-9" />
          <span className="font-semibold">{t("labelProgress.session.noPattern")}</span>
        </>
      ),
    };
  }
  // A saved id the loaded taxonomy no longer has still shows its name, without claiming an axis.
  const axis = taxonomy.find((pattern) => pattern.id === choice.pattern)?.axis;
  const accent = familyAccent(axis === undefined ? "" : axisFamily(axis));
  const key = axis === undefined ? undefined : axisKey(axis);
  return {
    className: cn(
      TILE_BASE,
      "bg-card bg-linear-to-r to-transparent pl-5",
      accent.card,
      "enabled:hover:border-control-border enabled:hover:bg-surface-raised",
    ),
    content: (
      <>
        <span aria-hidden="true" className={cn("absolute inset-y-0 left-0 w-1", accent.bar)} />
        <AxisIcon axis={axis ?? ""} className={cn("text-muted-foreground size-9", accent.icon)} />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-display text-base leading-tight font-bold break-words capitalize">
            {patternName(choice.pattern)}
          </span>
          {key !== undefined && (
            <span className="text-muted-foreground text-xs">{t(`label.axis.${key}`, { defaultValue: key })}</span>
          )}
        </span>
      </>
    ),
  };
}

/**
 * The session list's answer: a choice saves at once, since nothing nearby takes Enter and Undo is one press away.
 * The latest answer per chart wins (ADR 0020).
 */
export function SessionListAnswer(props: SessionListAnswerProps) {
  const { keymode, label, title, describedBy, taxonomy, children } = props;
  const { t } = useTranslation();
  const writer = useSessionAnswerWriter(props);
  const [sending, setSending] = useState<SessionAnswerChoice | null>(null);
  const shown: SessionAnswerChoice | null = sending ?? (label === null ? null : { pattern: label.pattern });
  const tile = useTile(shown, taxonomy);

  const send = async (pattern: string | null): Promise<void> => {
    // Re-sending the shown answer would only stack a duplicate event.
    if (writer.busy || (shown !== null && shown.pattern === pattern)) {
      return;
    }
    setSending({ pattern });
    await writer.save(pattern);
    setSending(null);
  };

  const choiceText =
    shown === null
      ? t("labelProgress.session.choose")
      : shown.pattern === null
        ? t("labelProgress.session.noPattern")
        : patternName(shown.pattern);
  return (
    <div className="flex flex-col gap-2">
      <PatternGridPicker
        keymode={keymode}
        taxonomy={taxonomy}
        disabled={writer.busy || taxonomy.length === 0}
        chosen={shown?.pattern ?? null}
        onChoose={(pattern) => {
          void send(pattern.id);
        }}
        title={t("labelProgress.session.pickerTitle", { title })}
        description={t("labelProgress.session.pickerDescription")}
        // The visible choice is part of the name (WCAG 2.5.3), so it is heard and can be spoken to.
        trigger={{ label: t("labelProgress.session.chooseFor", { title, choice: choiceText }), ...tile }}
      />
      <div className="flex flex-wrap items-center gap-1">
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-pressed={shown !== null && shown.pattern === null}
          aria-describedby={describedBy}
          disabled={writer.busy}
          onClick={() => {
            void send(null);
          }}
          className="aria-pressed:border-primary aria-pressed:bg-primary/15 aria-pressed:text-primary"
        >
          {t("labelProgress.session.noPattern")}
        </Button>
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
            {t("labelProgress.session.undo")}
          </Button>
        )}
        {children}
      </div>
      <SessionAnswerError error={writer.error} />
    </div>
  );
}
