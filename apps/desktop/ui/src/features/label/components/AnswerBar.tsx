import { Ban, Check, CircleSlash, SkipForward, Undo2, X } from "lucide-react";
import { type ReactNode, type Ref, useEffect, useEffectEvent, useId, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { PatternDefDto } from "@/ipc/bindings";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import type { Answer, FlagToggle } from "../session";
import { FlagToggles } from "./FlagToggles";
import { HoldButton, type HoldButtonHandle } from "./HoldButton";
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
  /** Whether the active hand layout has a thumb column, so the thumb-side flags apply. */
  thumbs: boolean;
  onSave: () => void;
  onClear: () => void;
  onUndo: () => void;
  onSkip: () => void;
  canSkip: boolean;
  /** Takes the viewer to the chip's pattern card in the panel; focus follows only for a keyboard activation. */
  onChipFocus: (patternId: string, how: { moveFocus: boolean }) => void;
  /** The bar's rendered height in px, so its scroll container can keep focused content clear of it. */
  onBlockSize?: (px: number) => void;
  /** How long Save and Skip must be held. */
  holdMs?: number;
  /** Lets the screen's Enter shortcut run Save's hold, ring included. */
  saveRef?: Ref<HoldButtonHandle>;
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

/** Fallback for `gap-1.5` when the computed style is unavailable. */
const SUMMARY_GAP_PX = 6;

interface SummaryItem {
  key: string;
  /** Spoken in the "+N" pill's name when the item is hidden. */
  name: string;
  node: ReactNode;
}

/** How many leading items fit in `available` px, reserving room for the "+N" pill whenever some are left out. */
function fitCount(widths: readonly number[], available: number, gap: number, more: number): number {
  // No layout (a hidden or not yet laid out line): show everything rather than collapse into "+N".
  if (available <= 0) {
    return widths.length;
  }
  let used = 0;
  for (const [i, width] of widths.entries()) {
    const next = used + (i > 0 ? gap : 0) + width;
    const rest = widths.length - i - 1;
    if (next + (rest > 0 ? gap + more : 0) > available) {
      return i;
    }
    used = next;
  }
  return widths.length;
}

/**
 * Measures every item off-screen and returns how many fit on the line. Re-measured when the items change and when
 * the line resizes (panel drag, window resize).
 */
function useFitCount(itemsKey: string, total: number) {
  const lineRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const [fit, setFit] = useState(total);

  const measure = useEffectEvent(() => {
    const line = lineRef.current;
    const ruler = measureRef.current;
    if (line === null || ruler === null) {
      return;
    }
    const widths = [...ruler.querySelectorAll<HTMLElement>("[data-fit=chip]")].map((el) => el.getBoundingClientRect().width);
    const more = ruler.querySelector<HTMLElement>("[data-fit=more]")?.getBoundingClientRect().width ?? 0;
    const gap = Number.parseFloat(getComputedStyle(line).columnGap);
    setFit(fitCount(widths, line.getBoundingClientRect().width, Number.isFinite(gap) ? gap : SUMMARY_GAP_PX, more));
  });

  useLayoutEffect(() => {
    measure();
  }, [itemsKey]);

  useEffect(() => {
    const line = lineRef.current;
    if (line === null) {
      return;
    }
    const observer = new ResizeObserver(() => {
      measure();
    });
    observer.observe(line);
    return () => {
      observer.disconnect();
    };
  }, []);

  return { lineRef, measureRef, fit: Math.min(fit, total) };
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
    thumbs,
    onSave,
    onClear,
    onUndo,
    onSkip,
    canSkip,
    onChipFocus,
    onBlockSize,
    holdMs,
    saveRef,
  } = props;
  const { t } = useTranslation();
  const hintId = useId();
  const undoReasonId = useId();
  const skipReasonId = useId();
  const editing = mode.kind === "edit";
  const locked = !editing || busy;
  const needsPattern = mode.kind === "edit" && !mode.canSave;
  const byId = new Map(taxonomy.map((p) => [p.id, p]));
  const ref = useBlockSize(onBlockSize);

  const flagBadges: string[] = [];
  if (answer.mixed) {
    flagBadges.push(t("label.flags.mixed"));
  }
  if (answer.unsure) {
    flagBadges.push(t("label.flags.unsure"));
  }
  if (answer.thumb !== null) {
    flagBadges.push(t(answer.thumb === "left" ? "label.flags.thumbLeft" : "label.flags.thumbRight"));
  }
  const badge = "border-border text-muted-foreground shrink-0 rounded-full border px-2 text-xs";

  const summaryItems: SummaryItem[] = [
    ...(answer.noPattern
      ? [
          {
            key: "noPattern",
            name: t("label.answer.noPatternChip"),
            node: (
              <span className="border-osu-yellow/60 bg-osu-yellow/10 text-osu-yellow shrink-0 rounded-full border px-2 font-semibold">
                {t("label.answer.noPatternChip")}
              </span>
            ),
          },
        ]
      : answer.patterns.map((id) => {
          const def = byId.get(id);
          return {
            key: id,
            name: patternName(id),
            node: (
              <span className="border-primary/60 bg-primary/10 inline-flex shrink-0 items-center gap-1 rounded-full border px-1.5 font-semibold">
                {def !== undefined && <kbd className="text-primary font-mono text-[0.7rem]">{def.key}</kbd>}
                <span className="capitalize">{patternName(id)}</span>
              </span>
            ),
          };
        })),
    ...flagBadges.map((label) => ({
      key: `flag:${label}`,
      name: label,
      node: <span className={badge}>{label}</span>,
    })),
  ];
  const showEmpty = !answer.noPattern && answer.patterns.length === 0;
  const itemsKey = summaryItems.map((item) => item.key).join("|");
  const { lineRef, measureRef, fit } = useFitCount(itemsKey, summaryItems.length);
  const shownItems = showEmpty ? summaryItems : summaryItems.slice(0, fit);
  const hiddenItems = showEmpty ? [] : summaryItems.slice(fit);

  return (
    // Fixed block size: the expanded panel overlays upward, so hover never reflows the grid or the scroll padding.
    <section
      ref={ref}
      aria-label={t("label.answer.label")}
      className="group sticky bottom-0 z-10 -mx-1 mt-auto h-14"
    >
      <div
        className={cn(
          "bg-surface-raised/95 absolute inset-x-0 bottom-0 flex flex-col rounded-t-xl border border-b-0 px-3 backdrop-blur",
          "shadow-[0_-16px_32px_-16px_rgb(0_0_0/0.7)]",
        )}
      >
        {/* Stays focusable while collapsed so focus-within can open it for keyboard users. */}
        <div
          data-testid="answer-expanded"
          className={cn(
            "pointer-events-none flex max-h-0 flex-col gap-2.5 overflow-hidden px-0.5 opacity-0 motion-safe:transition-[max-height,opacity,padding] motion-safe:duration-150",
            "group-focus-within:pointer-events-auto group-focus-within:max-h-64 group-focus-within:pt-3 group-focus-within:opacity-100",
            "group-hover:pointer-events-auto group-hover:max-h-64 group-hover:pt-3 group-hover:opacity-100",
            "group-has-[[aria-expanded=true]]:pointer-events-auto group-has-[[aria-expanded=true]]:max-h-64 group-has-[[aria-expanded=true]]:pt-3 group-has-[[aria-expanded=true]]:opacity-100",
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
                        className="border-primary/60 bg-primary/10 inline-flex h-8 items-center gap-0.5 rounded-full border pr-1 text-xs font-semibold"
                      >
                        <button
                          type="button"
                          aria-label={t("label.answer.focusChip", { name })}
                          title={t("label.answer.focusChip", { name })}
                          onClick={(e) => {
                            // detail is 0 for a click synthesised from Enter/Space: the keyboard user goes on from the
                            // card, while a pointer click keeps focus where it was so Enter still saves.
                            onChipFocus(id, { moveFocus: e.detail === 0 });
                          }}
                          className={cn(
                            "inline-flex h-full cursor-pointer items-center gap-1.5 rounded-full pr-1 pl-1.5 outline-none",
                            "hover:text-primary focus-visible:ring-ring focus-visible:ring-2 motion-safe:transition-colors",
                          )}
                        >
                          {def !== undefined && (
                            <kbd className="border-primary/60 text-primary rounded border px-1 font-mono text-[0.7rem] leading-4">
                              {def.key}
                            </kbd>
                          )}
                          <span className="capitalize">{name}</span>
                        </button>
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
            <FlagToggles flags={answer} disabled={locked} thumbs={thumbs} onToggle={onFlag} />
            {mode.kind !== "saved" && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                aria-keyshortcuts="Escape"
                disabled={locked}
                onClick={onClear}
                className="ml-auto"
              >
                {t("label.answer.clear")}
              </Button>
            )}
          </div>
        </div>

        <div className="flex h-14 items-center gap-2">
          {/* Decorative digest of the editable chips above; hidden while the panel is open to avoid doubling them. */}
          <div
            aria-hidden
            className={cn(
              "relative flex min-w-0 flex-1",
              "group-hover:invisible group-focus-within:invisible group-has-[[aria-expanded=true]]:invisible",
            )}
          >
            <div
              ref={lineRef}
              data-fit="line"
              data-testid="answer-summary"
              className="flex min-w-0 flex-1 items-center gap-1.5 overflow-hidden text-xs whitespace-nowrap"
            >
              {showEmpty && <span className="text-muted-foreground truncate">{t("label.answer.empty")}</span>}
              {shownItems.map((item) => (
                <span key={item.key} className="contents">
                  {item.node}
                </span>
              ))}
              {hiddenItems.length > 0 && (
                <span
                  data-fit="more"
                  data-testid="answer-overflow"
                  className="border-border bg-muted text-foreground shrink-0 rounded-full border px-1.5 font-semibold tabular-nums"
                >
                  +{hiddenItems.length}
                </span>
              )}
            </div>
            {/* Off-screen ruler: every item at its natural width, so the fit is known before anything is hidden. */}
            <div
              ref={measureRef}
              className="pointer-events-none invisible absolute top-0 left-0 flex gap-1.5 text-xs whitespace-nowrap"
            >
              {summaryItems.map((item) => (
                <span key={item.key} data-fit="chip" className="inline-flex shrink-0">
                  {item.node}
                </span>
              ))}
              <span data-fit="more" className="shrink-0 rounded-full border px-1.5 font-semibold tabular-nums">
                +{summaryItems.length}
              </span>
            </div>
          </div>

          {hiddenItems.length > 0 && (
            <span className="sr-only">
              {t("label.answer.more", {
                count: hiddenItems.length,
                names: hiddenItems.map((item) => item.name).join(", "),
              })}
            </span>
          )}
          <div className="max-w-[40%] min-w-0 shrink text-xs">
            {feedback !== null ? (
              <p
                role={feedback.tone === "error" ? "alert" : "status"}
                className={cn("truncate", feedback.tone === "error" ? "text-destructive" : "text-muted-foreground")}
                title={feedback.text}
              >
                {feedback.text}
              </p>
            ) : (
              needsPattern && (
                <p id={hintId} className="text-muted-foreground truncate" title={t("label.answer.needsPattern")}>
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
              className={cn("shrink-0", !mode.canUndo && "cursor-not-allowed opacity-45")}
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
            <HoldButton
              ref={saveRef}
              holdMs={holdMs}
              label={t("label.answer.submit")}
              icon={<Check aria-hidden />}
              variant="primary"
              disabled={locked || needsPattern}
              describedBy={needsPattern && feedback === null ? hintId : undefined}
              onConfirm={onSave}
              className="shrink-0"
            />
          )}
          <HoldButton
            holdMs={holdMs}
            label={t("label.answer.skip")}
            icon={<SkipForward aria-hidden />}
            variant="secondary"
            disabled={!canSkip || busy}
            describedBy={mode.kind === "saved" ? skipReasonId : undefined}
            onConfirm={onSkip}
            className="shrink-0"
          />
          {mode.kind === "saved" && (
            <span id={skipReasonId} hidden>
              {t("label.answer.savedNoSkip")}
            </span>
          )}
        </div>
      </div>
    </section>
  );
}
