import { ChevronDown, ChevronsDownUp, ChevronsUpDown, Search, SearchX } from "lucide-react";
import { type KeyboardEvent, type ReactNode, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { PatternDefDto } from "@/ipc/bindings";
import { type ChartWindow, PatternPreview } from "@/features/playfield";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { HoverCard, HoverCardContent, HoverCardTrigger } from "@/shared/ui/hover-card";
import { readOpenAxes, writeOpenAxes } from "./patternGridPrefs";
import { type AxisGroup, axisKey, groupByAxis, groupByFamily, matchesSearch, patternName } from "./patterns";

export interface PatternGridParams {
  /** Long enough that sweeping the pointer across the grid does not flash a card per cell. */
  previewOpenDelayMs: number;
  previewCloseDelayMs: number;
  /** Drawing size of the card thumbnail (3:4); CSS stretches it to the card width. */
  thumbnail: { width: number; height: number };
  enlarged: { width: number; height: number };
  /** Decorative examples on a collapsed axis card; more would crowd the title in a narrow panel. */
  stripCount: number;
  strip: { width: number; height: number };
}

export const PATTERN_GRID_PARAMS: PatternGridParams = {
  previewOpenDelayMs: 250,
  previewCloseDelayMs: 100,
  thumbnail: { width: 120, height: 160 },
  enlarged: { width: 200, height: 320 },
  stripCount: 3,
  strip: { width: 24, height: 36 },
};

interface FamilyAccent {
  heading: string;
  rule: string;
  card: string;
  bar: string;
  badge: string;
}

// Full class strings so Tailwind sees them; RICE and LN must read apart at a glance.
const FAMILY_ACCENTS: Readonly<Record<string, FamilyAccent>> = {
  regular: {
    heading: "text-osu-pink",
    rule: "from-osu-pink/70",
    card: "from-osu-pink/20 aria-expanded:border-osu-pink/60",
    bar: "bg-osu-pink",
    badge: "border-osu-pink/50 bg-osu-pink/15 text-osu-pink",
  },
  ln: {
    heading: "text-osu-blue",
    rule: "from-osu-blue/70",
    card: "from-osu-blue/20 aria-expanded:border-osu-blue/60",
    bar: "bg-osu-blue",
    badge: "border-osu-blue/50 bg-osu-blue/15 text-osu-blue",
  },
};

const FALLBACK_ACCENT: FamilyAccent = {
  heading: "text-primary",
  rule: "from-primary/70",
  card: "from-primary/20 aria-expanded:border-primary/60",
  bar: "bg-primary",
  badge: "border-primary/50 bg-primary/15 text-primary",
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
  const [openAxes, setOpenAxes] = useState(readOpenAxes);
  // A search shows its matches open; closing one there must not rewrite the viewer's saved layout.
  const [closedWhileSearching, setClosedWhileSearching] = useState<ReadonlySet<string>>(() => new Set());
  const searchRef = useRef<HTMLInputElement>(null);
  const needle = query.trim().toLowerCase();
  const searching = needle !== "";
  const fullAxes = new Map(groupByAxis(taxonomy).map((group) => [group.axis, group.patterns]));
  const groups = groupByAxis(taxonomy.filter((pattern) => matchesSearch(pattern, needle)));
  const families = groupByFamily(groups);
  const visibleAxes = groups.map((group) => group.axis);

  const isOpen = (axis: string): boolean => (searching ? !closedWhileSearching.has(axis) : openAxes.has(axis));

  const saveOpen = (next: ReadonlySet<string>): void => {
    setOpenAxes(next);
    writeOpenAxes(next);
  };

  const toggleAxis = (axis: string): void => {
    if (searching) {
      setClosedWhileSearching(toggled(closedWhileSearching, axis));
    } else {
      saveOpen(toggled(openAxes, axis));
    }
  };

  const setAll = (open: boolean): void => {
    if (searching) {
      setClosedWhileSearching(open ? new Set() : new Set(visibleAxes));
    } else {
      saveOpen(open ? new Set(visibleAxes) : new Set());
    }
  };

  const changeQuery = (next: string): void => {
    setQuery(next);
    setClosedWhileSearching(new Set());
  };
  const clearSearch = (): void => {
    changeQuery("");
    searchRef.current?.focus();
  };
  const onSearchKey = (e: KeyboardEvent<HTMLInputElement>): void => {
    if (e.key === "Escape" && query !== "") {
      e.preventDefault();
      changeQuery("");
    }
  };

  const allOpen = visibleAxes.length > 0 && visibleAxes.every(isOpen);
  const noneOpen = !visibleAxes.some(isOpen);

  return (
    <div className={cn("@container flex w-full min-w-0 flex-col gap-5", className)}>
      <div className="flex items-center gap-2">
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
              changeQuery(e.target.value);
            }}
            onKeyDown={onSearchKey}
            className={cn(
              "border-input bg-background h-9 w-full rounded-lg border pr-3 pl-8 text-sm outline-none",
              "placeholder:text-muted-foreground transition-[border-color,box-shadow] duration-150",
              "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-3",
            )}
          />
        </div>
        <Button
          variant="outline"
          size="icon"
          aria-label={t("label.patternGrid.expandAll")}
          title={t("label.patternGrid.expandAll")}
          disabled={allOpen || visibleAxes.length === 0}
          onClick={() => {
            setAll(true);
          }}
        >
          <ChevronsUpDown aria-hidden />
        </Button>
        <Button
          variant="outline"
          size="icon"
          aria-label={t("label.patternGrid.collapseAll")}
          title={t("label.patternGrid.collapseAll")}
          disabled={noneOpen}
          onClick={() => {
            setAll(false);
          }}
        >
          <ChevronsDownUp aria-hidden />
        </Button>
      </div>

      {families.length === 0 ? (
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
        families.map(({ family, axes }) => (
          <FamilySection key={family} family={family}>
            {axes.map((group) => (
              <AxisSection
                key={group.axis}
                group={group}
                allPatterns={fullAxes.get(group.axis) ?? group.patterns}
                accent={FAMILY_ACCENTS[family] ?? FALLBACK_ACCENT}
                open={isOpen(group.axis)}
                onToggleOpen={() => {
                  toggleAxis(group.axis);
                }}
                examples={examples}
                isActive={isActive}
                onToggle={onToggle}
              />
            ))}
          </FamilySection>
        ))
      )}
    </div>
  );
}

function toggled(set: ReadonlySet<string>, item: string): ReadonlySet<string> {
  const next = new Set(set);
  if (!next.delete(item)) {
    next.add(item);
  }
  return next;
}

function FamilySection({ family, children }: { family: string; children: ReactNode }) {
  const { t } = useTranslation();
  const headingId = useId();
  const accent = FAMILY_ACCENTS[family] ?? FALLBACK_ACCENT;
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-2.5">
      <h4
        id={headingId}
        className={cn("font-display flex items-center gap-3 text-2xl font-black tracking-[0.2em] uppercase", accent.heading)}
      >
        {t(`label.patternGrid.family.${family}`, { defaultValue: family.toUpperCase() })}
        <span aria-hidden className={cn("h-0.5 flex-1 rounded-full bg-linear-to-r to-transparent", accent.rule)} />
      </h4>
      {children}
    </section>
  );
}

interface AxisSectionProps {
  group: AxisGroup;
  /** The axis' whole taxonomy, so the count and selection badge ignore the search filter. */
  allPatterns: readonly PatternDefDto[];
  accent: FamilyAccent;
  open: boolean;
  onToggleOpen: () => void;
  examples: ReadonlyMap<string, ChartWindow> | undefined;
  isActive: (pattern: PatternDefDto) => boolean;
  onToggle: (pattern: PatternDefDto) => void;
}

function AxisSection({ group, allPatterns, accent, open, onToggleOpen, examples, isActive, onToggle }: AxisSectionProps) {
  const { t } = useTranslation();
  const titleId = useId();
  const countId = useId();
  const selectedId = useId();
  const panelId = useId();
  const key = axisKey(group.axis);
  const title = t(`label.axis.${key}`, { defaultValue: key });
  const selected = allPatterns.filter(isActive).length;
  const strip = allPatterns
    .map((pattern) => examples?.get(pattern.id))
    .filter((example): example is ChartWindow => example !== undefined)
    .slice(0, PATTERN_GRID_PARAMS.stripCount);
  const stripSize = PATTERN_GRID_PARAMS.strip;

  return (
    <div className="flex flex-col gap-2">
      <h5>
        <button
          type="button"
          data-axis={group.axis}
          aria-expanded={open}
          aria-controls={panelId}
          aria-labelledby={titleId}
          aria-describedby={selected > 0 ? `${countId} ${selectedId}` : countId}
          onClick={onToggleOpen}
          className={cn(
            "group bg-card relative flex min-h-16 w-full cursor-pointer items-center gap-3 overflow-hidden rounded-xl border py-3 pr-3 pl-5 text-left outline-none",
            "bg-linear-to-r to-transparent",
            accent.card,
            "hover:border-control-border hover:bg-surface-raised",
            "motion-safe:transition-[background-color,border-color,box-shadow] motion-safe:duration-150 motion-safe:ease-out",
            "focus-visible:border-ring focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-2",
          )}
        >
          <span aria-hidden className={cn("absolute inset-y-0 left-0 w-1", accent.bar)} />
          <span className="flex min-w-0 flex-1 flex-col gap-1">
            <span id={titleId} className="font-display truncate text-xl leading-none font-extrabold tracking-widest uppercase">
              {title}
            </span>
            <span className="flex items-center gap-2 text-xs">
              <span id={countId} className="text-muted-foreground">
                {t("label.patternGrid.patternCount", { count: allPatterns.length })}
              </span>
              {selected > 0 && (
                <span id={selectedId} className={cn("rounded-full border px-1.5 leading-4 font-semibold", accent.badge)}>
                  {t("label.patternGrid.selectedCount", { count: selected })}
                </span>
              )}
            </span>
          </span>
          {strip.length > 0 && (
            <span aria-hidden className="hidden shrink-0 gap-1 opacity-80 @xs:flex">
              {strip.map((example, i) => (
                <PatternPreview
                  key={i}
                  window={example}
                  width={stripSize.width}
                  height={stripSize.height}
                  label=""
                  className="w-6 rounded-sm"
                />
              ))}
            </span>
          )}
          <ChevronDown
            aria-hidden
            className="text-muted-foreground size-5 shrink-0 motion-safe:transition-transform motion-safe:duration-150 group-aria-expanded:rotate-180"
          />
        </button>
      </h5>
      <div id={panelId} role="region" aria-labelledby={titleId} hidden={!open}>
        {/* Closed panels mount no cards: a full taxonomy would otherwise draw every preview canvas up front. */}
        {open && (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(6.5rem,1fr))] gap-2">
            {group.patterns.map((pattern) => (
              <PatternCard
                key={pattern.id}
                pattern={pattern}
                example={examples?.get(pattern.id)}
                loading={examples === undefined}
                active={isActive(pattern)}
                onToggle={onToggle}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

interface PatternCardProps {
  pattern: PatternDefDto;
  example: ChartWindow | undefined;
  loading: boolean;
  active: boolean;
  onToggle: (pattern: PatternDefDto) => void;
}

function PatternCard({ pattern, example, loading, active, onToggle }: PatternCardProps) {
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
            "group flex cursor-pointer flex-col gap-1.5 rounded-lg border p-1.5 text-left outline-none",
            "motion-safe:transition-[background-color,border-color,box-shadow,transform] motion-safe:duration-150 motion-safe:ease-out",
            "focus-visible:border-ring focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-2",
            "motion-safe:hover:-translate-y-0.5 motion-safe:active:translate-y-0",
            active
              ? "border-primary bg-primary/10 hover:bg-primary/15 shadow-[0_0_14px_-6px_var(--primary)]"
              : "border-border bg-card hover:border-control-border hover:bg-surface-raised",
          )}
        >
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
