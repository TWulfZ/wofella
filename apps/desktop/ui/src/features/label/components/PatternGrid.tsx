import { Search, SearchX } from "lucide-react";
import { type KeyboardEvent, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { PatternDefDto } from "@/ipc/bindings";
import { type ChartWindow, PatternPreview } from "@/features/playfield";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { Checkbox } from "@/shared/ui/checkbox";
import { HoverCard, HoverCardContent, HoverCardTrigger } from "@/shared/ui/hover-card";
import { readAlwaysCollapsed, writeAlwaysCollapsed } from "./patternGridPrefs";
import { axisKey, groupByAxis, matchesSearch, patternName } from "./patterns";

/** Expanded shows preview cards; compact trades them for wrapping pills so a search reads across the row. */
export type PatternGridDensity = "expanded" | "compact";

export interface PatternGridParams {
  /** Long enough that sweeping the pointer across the grid does not flash a card per cell. */
  previewOpenDelayMs: number;
  previewCloseDelayMs: number;
  /** Drawing size of the card thumbnail (3:4); CSS stretches it to the card width. */
  thumbnail: { width: number; height: number };
  enlarged: { width: number; height: number };
}

export const PATTERN_GRID_PARAMS: PatternGridParams = {
  previewOpenDelayMs: 250,
  previewCloseDelayMs: 100,
  thumbnail: { width: 120, height: 160 },
  enlarged: { width: 200, height: 320 },
};

interface PatternGridProps {
  taxonomy: readonly PatternDefDto[];
  /** Undefined while the examples load; a loaded map may still miss an id. */
  examples: ReadonlyMap<string, ChartWindow> | undefined;
  isActive: (pattern: PatternDefDto) => boolean;
  onToggle: (pattern: PatternDefDto) => void;
  className?: string;
}

export function PatternGrid({ taxonomy, examples, isActive, onToggle, className }: PatternGridProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [alwaysCollapsed, setAlwaysCollapsed] = useState(readAlwaysCollapsed);
  const searchRef = useRef<HTMLInputElement>(null);
  const collapseId = useId();
  const needle = query.trim().toLowerCase();
  const groups = groupByAxis(taxonomy.filter((pattern) => matchesSearch(pattern, needle)));
  // Raw query, not the trimmed needle: a typed space already signals a search in progress.
  const density: PatternGridDensity = alwaysCollapsed || query !== "" ? "compact" : "expanded";
  const compact = density === "compact";

  const clearSearch = (): void => {
    setQuery("");
    searchRef.current?.focus();
  };
  const onSearchKey = (e: KeyboardEvent<HTMLInputElement>): void => {
    if (e.key === "Escape" && query !== "") {
      e.preventDefault();
      setQuery("");
    }
  };

  const onCollapseChange = (checked: boolean | "indeterminate"): void => {
    const next = checked === true;
    setAlwaysCollapsed(next);
    writeAlwaysCollapsed(next);
  };

  return (
    <div className={cn("flex w-full min-w-0 flex-col", compact ? "gap-3" : "gap-5", className)}>
      <div className="flex items-center gap-3">
        <div className="relative min-w-0 flex-1">
          <Search
            aria-hidden
            className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
          />
          <input
            ref={searchRef}
            type="search"
            value={query}
            aria-label={t("label.patternGrid.search")}
            placeholder={t("label.patternGrid.searchPlaceholder")}
            autoComplete="off"
            spellCheck={false}
            onChange={(e) => {
              setQuery(e.target.value);
            }}
            onKeyDown={onSearchKey}
            className={cn(
              "border-input bg-background h-9 w-full rounded-lg border pr-3 pl-8 text-sm outline-none",
              "placeholder:text-muted-foreground transition-[border-color,box-shadow] duration-150",
              "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-3",
            )}
          />
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Checkbox id={collapseId} checked={alwaysCollapsed} onCheckedChange={onCollapseChange} />
          <label
            htmlFor={collapseId}
            className="text-muted-foreground cursor-pointer text-xs whitespace-nowrap select-none"
          >
            {t("label.patternGrid.alwaysCollapsed")}
          </label>
        </div>
      </div>

      {groups.length === 0 ? (
        <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed px-4 py-6 text-center">
          <SearchX aria-hidden className="text-muted-foreground size-5" />
          <p role="status" className="text-sm font-medium">
            {t("label.patternGrid.noResults", { query: query.trim() })}
          </p>
          <p className="text-muted-foreground text-xs">{t("label.patternGrid.noResultsHint")}</p>
          <Button variant="outline" size="sm" onClick={clearSearch}>
            {t("label.patternGrid.clearSearch")}
          </Button>
        </div>
      ) : (
        groups.map(({ axis, patterns }) => {
          const key = axisKey(axis);
          const title = t(`label.axis.${key}`, { defaultValue: key });
          return (
            <section key={axis} aria-label={title} className={cn("flex flex-col", compact ? "gap-1.5" : "gap-3")}>
              <h4
                className={cn(
                  "font-display flex items-center tracking-widest uppercase",
                  compact
                    ? "text-muted-foreground gap-2 text-xs font-bold"
                    : "text-foreground gap-3 text-xl font-extrabold",
                )}
              >
                {title}
                <span
                  aria-hidden
                  className={cn(
                    "flex-1 rounded-full bg-linear-to-r to-transparent",
                    compact ? "from-primary/40 h-px" : "from-primary/70 h-0.5",
                  )}
                />
              </h4>
              <div
                className={
                  compact ? "flex flex-wrap gap-1.5" : "grid grid-cols-[repeat(auto-fill,minmax(6.5rem,1fr))] gap-2"
                }
              >
                {patterns.map((pattern) => (
                  <PatternCard
                    key={pattern.id}
                    pattern={pattern}
                    example={examples?.get(pattern.id)}
                    loading={examples === undefined}
                    active={isActive(pattern)}
                    compact={compact}
                    onToggle={onToggle}
                  />
                ))}
              </div>
            </section>
          );
        })
      )}
    </div>
  );
}

interface PatternCardProps {
  pattern: PatternDefDto;
  example: ChartWindow | undefined;
  loading: boolean;
  active: boolean;
  compact: boolean;
  onToggle: (pattern: PatternDefDto) => void;
}

function PatternCard({ pattern, example, loading, active, compact, onToggle }: PatternCardProps) {
  const { t } = useTranslation();
  const descriptionId = useId();
  const name = patternName(pattern.id);
  const { thumbnail, enlarged } = PATTERN_GRID_PARAMS;

  return (
    <HoverCard openDelay={PATTERN_GRID_PARAMS.previewOpenDelayMs} closeDelay={PATTERN_GRID_PARAMS.previewCloseDelayMs}>
      <HoverCardTrigger asChild>
        <button
          type="button"
          aria-pressed={active}
          // Same name as the chip it replaces ("<key> <name>"), whatever the visual order.
          aria-label={`${pattern.key} ${name}`}
          aria-describedby={descriptionId}
          onClick={() => {
            onToggle(pattern);
          }}
          className={cn(
            "group cursor-pointer border text-left outline-none",
            "motion-safe:transition-[background-color,border-color,box-shadow,transform] motion-safe:duration-150 motion-safe:ease-out",
            "focus-visible:border-ring focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-2",
            compact
              ? "inline-flex h-7 max-w-full items-center gap-1.5 rounded-full pr-2.5 pl-1"
              : "flex flex-col gap-1.5 rounded-lg p-1.5 motion-safe:hover:-translate-y-0.5 motion-safe:active:translate-y-0",
            active
              ? "border-primary bg-primary/10 hover:bg-primary/15 shadow-[0_0_14px_-6px_var(--primary)]"
              : "border-border bg-card hover:border-control-border hover:bg-surface-raised",
          )}
        >
          {compact ? (
            <>
              <PatternKey active={active}>{pattern.key}</PatternKey>
              <span className="truncate text-xs leading-tight font-semibold capitalize">{name}</span>
            </>
          ) : (
            <>
              <div aria-hidden className="w-full">
                {example === undefined ? (
                  <PreviewPlaceholder loading={loading} className="aspect-[3/4] w-full" />
                ) : (
                  <PatternPreview window={example} width={thumbnail.width} height={thumbnail.height} label={name} />
                )}
              </div>
              <span className="flex w-full items-center justify-between gap-1">
                <span className="truncate text-xs leading-tight font-semibold capitalize">{name}</span>
                <PatternKey active={active}>{pattern.key}</PatternKey>
              </span>
            </>
          )}
          <span id={descriptionId} hidden>
            {pattern.description}
          </span>
        </button>
      </HoverCardTrigger>
      <HoverCardContent side="left" align="start" collisionPadding={8} className="flex w-auto flex-col gap-2 p-3">
        {example === undefined ? (
          <PreviewPlaceholder loading={loading} className="aspect-[5/8] w-[200px]" />
        ) : (
          <PatternPreview
            window={example}
            width={enlarged.width}
            height={enlarged.height}
            label={t("label.patternGrid.preview", { name })}
            className="w-[200px]"
          />
        )}
        <div className="flex w-[200px] items-center justify-between gap-2">
          <span className="font-display truncate text-base font-bold capitalize">{name}</span>
          <PatternKey active={active}>{pattern.key}</PatternKey>
        </div>
        <p className="text-muted-foreground w-[200px] text-sm leading-snug">{pattern.description}</p>
      </HoverCardContent>
    </HoverCard>
  );
}

function PatternKey({ active, children }: { active: boolean; children: string }) {
  return (
    <kbd
      className={cn(
        "shrink-0 rounded border px-1 font-mono text-[0.7rem] leading-4 font-semibold",
        active ? "border-primary/60 text-primary" : "border-border text-muted-foreground",
      )}
    >
      {children}
    </kbd>
  );
}

function PreviewPlaceholder({ loading, className }: { loading: boolean; className: string }) {
  return <div className={cn("bg-muted rounded-md", loading && "motion-safe:animate-pulse", className)} />;
}
