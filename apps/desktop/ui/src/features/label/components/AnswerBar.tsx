import { Ban, Check, CircleSlash, Undo2, X } from "lucide-react";
import { useEffect, useEffectEvent, useId, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { PatternDefDto } from "@/ipc/bindings";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import type { Answer, FlagToggle } from "../session";
import { FlagToggles } from "./FlagToggles";
import { patternName } from "./patterns";
import { PatternPicker } from "./PatternPicker";

export type AnswerMode =
  | { kind: "none" }
  | { kind: "edit"; skipped: boolean; canSave: boolean }
  | { kind: "saved"; canUndo: boolean };

export interface AnswerFeedback {
  tone: "error" | "info";
  text: string;
}

interface AnswerBarProps {
  taxonomy: readonly PatternDefDto[];
  /** The draft while editing, the stored answer on a saved window. */
  answer: Answer;
  mode: AnswerMode;
  busy: boolean;
  feedback: AnswerFeedback | null;
  onPick: (pattern: PatternDefDto) => void;
  onRemove: (id: string) => void;
  onNoPattern: () => void;
  onFlag: (toggle: FlagToggle) => void;
  onSave: () => void;
  onClear: () => void;
  onUndo: () => void;
  /** The bar's rendered height in px, so its scroll container can keep focused content clear of it. */
  onBlockSize?: (px: number) => void;
}

function useBlockSize(onBlockSize: ((px: number) => void) | undefined) {
  const ref = useRef<HTMLElement>(null);
  const report = useEffectEvent((px: number) => {
    onBlockSize?.(px);
  });
  useEffect(() => {
    const el = ref.current;
    if (el === null) {
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

export function AnswerBar(props: AnswerBarProps) {
  const {
    taxonomy,
    answer,
    mode,
    busy,
    feedback,
    onPick,
    onRemove,
    onNoPattern,
    onFlag,
    onSave,
    onClear,
    onUndo,
    onBlockSize,
  } = props;
  const { t } = useTranslation();
  const hintId = useId();
  const undoReasonId = useId();
  const editing = mode.kind === "edit";
  const locked = !editing || busy;
  const needsPattern = mode.kind === "edit" && !mode.canSave;
  const byId = new Map(taxonomy.map((p) => [p.id, p]));
  const ref = useBlockSize(onBlockSize);

  return (
    <section
      ref={ref}
      aria-label={t("label.answer.label")}
      className={cn(
        "bg-surface-raised/95 sticky bottom-0 z-10 -mx-1 mt-auto flex flex-col gap-2.5 rounded-t-xl border border-b-0 px-3 pt-3 pb-3",
        "shadow-[0_-16px_32px_-16px_rgb(0_0_0/0.7)] backdrop-blur",
      )}
    >
      <div className="flex items-center gap-2">
        <h3 className="font-display text-xs font-bold tracking-widest uppercase">{t("label.answer.label")}</h3>
        {mode.kind === "saved" && (
          <span className="border-success/50 text-success inline-flex items-center gap-1 rounded-full border px-2 text-xs font-semibold">
            <Check aria-hidden className="size-3" />
            {t("label.answer.saved")}
          </span>
        )}
        {mode.kind === "edit" && mode.skipped && (
          <span className="border-border text-muted-foreground inline-flex items-center gap-1 rounded-full border px-2 text-xs">
            {t("label.answer.skipped")}
          </span>
        )}
      </div>

      <div className="flex min-h-9 items-start gap-2">
        <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
          {answer.noPattern ? (
            <span className="border-osu-yellow/60 bg-osu-yellow/10 text-osu-yellow inline-flex h-8 items-center gap-1.5 rounded-full border px-3 text-xs font-semibold">
              <CircleSlash aria-hidden className="size-3.5" />
              {t("label.answer.noPatternChip")}
            </span>
          ) : answer.patterns.length === 0 ? (
            <p className="text-muted-foreground py-2 text-xs">{t("label.answer.empty")}</p>
          ) : (
            <ul aria-label={t("label.answer.selected")} className="flex flex-wrap gap-1.5">
              {answer.patterns.map((id) => {
                const def = byId.get(id);
                const name = patternName(id);
                return (
                  <li
                    key={id}
                    data-pattern-id={id}
                    className="border-primary/60 bg-primary/10 inline-flex h-8 items-center gap-1.5 rounded-full border pr-1 pl-1.5 text-xs font-semibold"
                  >
                    {def !== undefined && (
                      <kbd className="border-primary/60 text-primary rounded border px-1 font-mono text-[0.7rem] leading-4">
                        {def.key}
                      </kbd>
                    )}
                    <span className="capitalize">{name}</span>
                    {editing ? (
                      <button
                        type="button"
                        aria-label={t("label.answer.remove", { name })}
                        disabled={busy}
                        onClick={() => {
                          onRemove(id);
                        }}
                        className={cn(
                          "text-muted-foreground hover:bg-primary/20 hover:text-foreground grid size-6 place-items-center rounded-full outline-none",
                          "focus-visible:ring-ring focus-visible:ring-2 motion-safe:transition-colors",
                        )}
                      >
                        <X aria-hidden className="size-3.5" />
                      </button>
                    ) : (
                      <span className="w-1" />
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </div>
        <PatternPicker
          taxonomy={taxonomy}
          isChosen={(pattern) => answer.patterns.includes(pattern.id)}
          onPick={onPick}
          disabled={locked}
        />
      </div>

      <div className="flex flex-wrap items-center gap-1">
        <Button
          type="button"
          size="sm"
          variant="outline"
          aria-pressed={answer.noPattern}
          disabled={locked}
          onClick={onNoPattern}
          className={cn(
            "rounded-full",
            answer.noPattern &&
              "border-osu-yellow/70 bg-osu-yellow/15 text-osu-yellow hover:bg-osu-yellow/20 hover:text-osu-yellow",
          )}
        >
          <Ban aria-hidden />
          {t("label.answer.noPattern")}
        </Button>
        <span aria-hidden className="bg-border mx-1 h-5 w-px" />
        <FlagToggles flags={answer} disabled={locked} onToggle={onFlag} />
      </div>

      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1 text-xs">
          {feedback !== null ? (
            <p
              role={feedback.tone === "error" ? "alert" : "status"}
              className={feedback.tone === "error" ? "text-destructive" : "text-muted-foreground"}
            >
              {feedback.text}
            </p>
          ) : (
            needsPattern && (
              <p id={hintId} className="text-muted-foreground">
                {t("label.answer.needsPattern")}
              </p>
            )
          )}
        </div>
        {mode.kind === "saved" ? (
          <Button
            type="button"
            variant="outline"
            size="lg"
            aria-disabled={!mode.canUndo || busy}
            aria-describedby={mode.canUndo ? undefined : undoReasonId}
            title={mode.canUndo ? undefined : t("label.answer.undoNotNewest")}
            onClick={() => {
              if (mode.canUndo && !busy) {
                onUndo();
              }
            }}
            className={cn(!mode.canUndo && "cursor-not-allowed opacity-45")}
          >
            <Undo2 aria-hidden />
            {t("label.answer.undo")}
            {!mode.canUndo && (
              <span id={undoReasonId} hidden>
                {t("label.answer.undoNotNewest")}
              </span>
            )}
          </Button>
        ) : (
          <>
            <Button
              type="button"
              variant="ghost"
              size="lg"
              aria-keyshortcuts="Escape"
              disabled={locked}
              onClick={onClear}
            >
              {t("label.answer.clear")}
            </Button>
            <Button
              type="button"
              size="lg"
              aria-keyshortcuts="Enter"
              aria-describedby={needsPattern && feedback === null ? hintId : undefined}
              disabled={locked || needsPattern}
              onClick={onSave}
              className="min-w-24"
            >
              {t("label.answer.submit")}
            </Button>
          </>
        )}
      </div>
    </section>
  );
}
