import { Link, useLocation } from "@tanstack/react-router";
import { ChevronDown } from "lucide-react";
import { type FocusEvent, type KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";
import { NAV_PARAMS, type NavBadge, type NavGroupEntry, type NavLinkEntry, type NavRequirement } from "./nav";

export type NavBadgeCounts = Partial<Record<NavBadge, number>>;

/** Why each unmet requirement disables its entries; a met one is absent. */
export type NavUnmet = Partial<Record<NavRequirement, string>>;

const NAV_ITEM_CLASS =
  "text-muted-foreground hover:text-foreground focus-visible:ring-ring data-[status=active]:text-foreground data-[status=active]:after:bg-primary relative inline-flex h-14 items-center gap-2 rounded-sm px-3 text-sm font-medium transition-colors duration-200 after:absolute after:inset-x-2 after:bottom-0 after:hidden after:h-[3px] after:rounded-full focus-visible:ring-2 focus-visible:outline-none data-[status=active]:after:block";

function badgeCount(badge: NavBadge | undefined, counts: NavBadgeCounts): number {
  return badge === undefined ? 0 : (counts[badge] ?? 0);
}

function PendingBadge({ count }: { count: number }) {
  if (count <= 0) {
    return null;
  }
  return (
    <span
      aria-hidden="true"
      data-testid="pending-badge"
      className="bg-primary text-primary-foreground grid h-4 min-w-4 place-items-center rounded-full px-1 text-[0.65rem] leading-none font-bold tabular-nums"
    >
      {count > NAV_PARAMS.badgeMax ? `${NAV_PARAMS.badgeMax}+` : count}
    </span>
  );
}

/** The visible text plus the count, spelled out once, since the badge itself is hidden from assistive tech. */
function useEntryName(labelKey: string, count: number): { label: string; name: string | undefined } {
  const { t } = useTranslation();
  const label = t(labelKey);
  return { label, name: count > 0 ? t("common.nav.withPending", { label, count }) : undefined };
}

export function NavLinkItem({ entry, counts }: { entry: NavLinkEntry; counts: NavBadgeCounts }) {
  const { to, labelKey, icon: Icon, badge } = entry;
  const count = badgeCount(badge, counts);
  const { label, name } = useEntryName(labelKey, count);
  return (
    <Link
      to={to}
      search={(prev) => prev}
      activeOptions={{ exact: true, includeSearch: false }}
      aria-label={name}
      className={NAV_ITEM_CLASS}
    >
      <Icon className="size-4" aria-hidden="true" />
      {label}
      <PendingBadge count={count} />
    </Link>
  );
}

/**
 * The W3C disclosure navigation pattern rather than an ARIA menu: the entries are plain links reached with Tab, and
 * Escape, a click outside or focus leaving the group closes it.
 */
export function NavGroupMenu({
  entry,
  counts,
  unmet = {},
}: {
  entry: NavGroupEntry;
  counts: NavBadgeCounts;
  unmet?: NavUnmet;
}) {
  const { t } = useTranslation();
  const { labelKey, icon: Icon, badge, items, pathPrefix, requires } = entry;
  const [open, setOpen] = useState(false);
  const listId = useId();
  const reasonId = useId();
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const pathname = useLocation({ select: (l) => l.pathname });
  const active = pathname === pathPrefix || pathname.startsWith(`${pathPrefix}/`);
  const reason = requires === undefined ? undefined : unmet[requires];
  const disabled = reason !== undefined;
  // A pending count invites work the disabled area cannot take.
  const count = disabled ? 0 : badgeCount(badge, counts);
  const { label, name } = useEntryName(labelKey, count);
  const expanded = open && !disabled;

  useEffect(() => {
    if (!expanded) {
      return;
    }
    const onPointerDown = (e: PointerEvent): void => {
      if (e.target instanceof Node && root.current?.contains(e.target) !== true) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [expanded]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>): void => {
    if (e.key === "Escape" && expanded) {
      e.preventDefault();
      setOpen(false);
      button.current?.focus();
    }
  };
  const onBlur = (e: FocusEvent<HTMLDivElement>): void => {
    if (!(e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget))) {
      setOpen(false);
    }
  };

  return (
    <div ref={root} onKeyDown={onKeyDown} onBlur={onBlur} className="relative h-full">
      {/* aria-disabled rather than disabled, so the entry stays focusable and hoverable and its reason can be read. */}
      {/* Always wrapped, so an entry that becomes disabled keeps its node and focus. */}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            ref={button}
            type="button"
            aria-expanded={expanded}
            aria-controls={listId}
            aria-label={name}
            aria-disabled={disabled || undefined}
            aria-describedby={disabled ? reasonId : undefined}
            data-active={active}
            data-status={active ? "active" : undefined}
            onClick={() => {
              if (!disabled) {
                setOpen((o) => !o);
              }
            }}
            className={cn(NAV_ITEM_CLASS, disabled ? "cursor-not-allowed opacity-45 hover:text-muted-foreground" : "cursor-pointer")}
          >
            <Icon className="size-4" aria-hidden="true" />
            {label}
            <PendingBadge count={count} />
            <ChevronDown
              aria-hidden="true"
              className={cn("size-3.5 motion-safe:transition-transform motion-safe:duration-200", expanded && "rotate-180")}
            />
            {disabled && (
              <span id={reasonId} hidden>
                {reason}
              </span>
            )}
          </button>
        </TooltipTrigger>
        {disabled && <TooltipContent side="bottom">{reason}</TooltipContent>}
      </Tooltip>
      {expanded && (
        <ul
          id={listId}
          aria-label={t("common.nav.sections", { label })}
          className="bg-popover text-popover-foreground motion-safe:animate-in motion-safe:fade-in-0 motion-safe:slide-in-from-top-1 absolute top-full left-0 z-30 mt-1 flex min-w-48 flex-col gap-0.5 rounded-lg border p-1 shadow-lg shadow-black/30"
        >
          {items.map((item) => (
            <li key={item.to}>
              <NavMenuLink
                entry={item}
                counts={counts}
                onNavigate={() => {
                  setOpen(false);
                }}
              />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function NavMenuLink({ entry, counts, onNavigate }: { entry: NavLinkEntry; counts: NavBadgeCounts; onNavigate: () => void }) {
  const { to, labelKey, icon: Icon, badge } = entry;
  const count = badgeCount(badge, counts);
  const { label, name } = useEntryName(labelKey, count);
  return (
    <Link
      to={to}
      search={(prev) => prev}
      activeOptions={{ exact: true, includeSearch: false }}
      aria-label={name}
      onClick={onNavigate}
      className="text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:ring-ring data-[status=active]:bg-primary/15 data-[status=active]:text-foreground flex items-center gap-2 rounded-md px-2.5 py-2 text-sm font-medium transition-colors duration-150 focus-visible:ring-2 focus-visible:outline-none"
    >
      <Icon className="size-4" aria-hidden="true" />
      {label}
      <span className="ml-auto">
        <PendingBadge count={count} />
      </span>
    </Link>
  );
}
