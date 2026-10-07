import { Check, Plus, Search } from "lucide-react";
import { type KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { PatternDefDto } from "@/ipc/bindings";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/shared/ui/popover";
import { axisKey, matchesSearch, patternName } from "./patterns";

interface PatternPickerProps {
  taxonomy: readonly PatternDefDto[];
  isChosen: (pattern: PatternDefDto) => boolean;
  onPick: (pattern: PatternDefDto) => void;
  disabled: boolean;
}

/** The only way besides the grid to put a pattern in the answer: a filter over the taxonomy, never free text. */
export function PatternPicker({ taxonomy, isChosen, onPick, disabled }: PatternPickerProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const listId = useId();
  const needle = query.trim().toLowerCase();
  const matches = taxonomy.filter((pattern) => matchesSearch(pattern, needle));
  const activeIndex = Math.min(active, Math.max(matches.length - 1, 0));
  const activePattern = matches[activeIndex];
  const optionId = (index: number): string => `${listId}-option-${index}`;

  useEffect(() => {
    const list = listRef.current;
    const option = list?.querySelector<HTMLElement>(`[data-index="${activeIndex}"]`);
    if (list === null || option === null || option === undefined) {
      return;
    }
    // Manual, since jsdom and some webviews lack a scrollIntoView that respects a nested scroller.
    if (option.offsetTop < list.scrollTop) {
      list.scrollTop = option.offsetTop;
    } else if (option.offsetTop + option.offsetHeight > list.scrollTop + list.clientHeight) {
      list.scrollTop = option.offsetTop + option.offsetHeight - list.clientHeight;
    }
  }, [activeIndex]);

  const onOpenChange = (next: boolean): void => {
    setOpen(next);
    if (!next) {
      setQuery("");
      setActive(0);
    }
  };

  const pick = (pattern: PatternDefDto): void => {
    onPick(pattern);
    setQuery("");
    setActive(0);
    inputRef.current?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>): void => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const last = Math.max(matches.length - 1, 0);
      setActive(e.key === "ArrowDown" ? Math.min(activeIndex + 1, last) : Math.max(activeIndex - 1, 0));
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      setActive(e.key === "Home" ? 0 : Math.max(matches.length - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (!e.repeat && activePattern !== undefined) {
        pick(activePattern);
      }
    }
  };

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="outline"
          size="icon-lg"
          aria-label={t("label.answer.add")}
          disabled={disabled}
          className="border-dashed motion-safe:transition-colors"
        >
          <Plus aria-hidden />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        side="top"
        align="start"
        collisionPadding={8}
        className="flex w-80 flex-col p-0"
        onOpenAutoFocus={(e) => {
          e.preventDefault();
          inputRef.current?.focus();
        }}
        // Focus falls back to the page, not the + button, so the next Enter saves instead of reopening the picker.
        onCloseAutoFocus={(e) => {
          e.preventDefault();
        }}
      >
        <div className="relative border-b p-2">
          <Search
            aria-hidden
            className="text-muted-foreground pointer-events-none absolute top-1/2 left-4.5 size-4 -translate-y-1/2"
          />
          <input
            ref={inputRef}
            role="combobox"
            aria-expanded
            aria-controls={listId}
            aria-autocomplete="list"
            aria-activedescendant={activePattern === undefined ? undefined : optionId(activeIndex)}
            aria-label={t("label.answer.find")}
            placeholder={t("label.answer.findPlaceholder")}
            autoComplete="off"
            spellCheck={false}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
            onKeyDown={onKeyDown}
            className={cn(
              "border-input bg-background h-9 w-full rounded-md border pr-2 pl-8 text-sm outline-none",
              "placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-3",
            )}
          />
        </div>
        <ul
          ref={listRef}
          id={listId}
          role="listbox"
          aria-label={t("label.answer.options")}
          className="relative max-h-72 overflow-y-auto p-1"
        >
          {matches.map((pattern, index) => {
            const name = patternName(pattern.id);
            const chosen = isChosen(pattern);
            const axis = axisKey(pattern.axis);
            return (
              <li
                key={pattern.id}
                id={optionId(index)}
                data-index={index}
                role="option"
                aria-label={`${pattern.key} ${name}`}
                aria-selected={index === activeIndex}
                aria-checked={chosen}
                onPointerMove={() => {
                  setActive(index);
                }}
                onClick={() => {
                  pick(pattern);
                }}
                className={cn(
                  "flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-sm",
                  index === activeIndex ? "bg-muted text-foreground" : "text-muted-foreground",
                )}
              >
                <kbd
                  className={cn(
                    "w-9 shrink-0 rounded border px-1 text-center font-mono text-[0.7rem] leading-4 font-semibold",
                    chosen ? "border-primary/60 text-primary" : "border-border",
                  )}
                >
                  {pattern.key}
                </kbd>
                <span className="text-foreground min-w-0 flex-1 truncate font-medium capitalize">{name}</span>
                <span className="text-muted-foreground shrink-0 text-[0.7rem] tracking-wide uppercase">
                  {t(`label.axis.${axis}`, { defaultValue: axis })}
                </span>
                <Check aria-hidden className={cn("text-primary size-4 shrink-0", !chosen && "invisible")} />
              </li>
            );
          })}
        </ul>
        {matches.length === 0 && (
          <p role="status" className="text-muted-foreground px-3 pb-3 text-sm">
            {t("label.patternGrid.noResults", { query: query.trim() })}
          </p>
        )}
      </PopoverContent>
    </Popover>
  );
}
