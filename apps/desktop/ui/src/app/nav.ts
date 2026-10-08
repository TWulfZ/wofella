import type { LinkProps } from "@tanstack/react-router";
import { ChartColumnBig, Compass, House, type LucideIcon, Radar, Settings, Tags, UserRound } from "lucide-react";

/** A count the shell shows next to an entry; each kind maps to one query in the shell. */
export type NavBadge = "sessionPending";

/** A capability of the active keymode an area needs; without it the shell disables the entry and says why. */
export type NavRequirement = "patterns";

export interface NavLinkEntry {
  kind: "link";
  to: NonNullable<LinkProps["to"]>;
  labelKey: string;
  icon: LucideIcon;
  badge?: NavBadge;
}

/** A section whose pages open from one disclosure button, so the bar keeps one entry per area. */
export interface NavGroupEntry {
  kind: "group";
  /** Pages under it count as its own for the active marker. */
  pathPrefix: string;
  labelKey: string;
  icon: LucideIcon;
  badge?: NavBadge;
  requires?: NavRequirement;
  items: readonly NavLinkEntry[];
}

export type NavEntry = NavLinkEntry | NavGroupEntry;

export const NAV_PARAMS = {
  /** Beyond it the badge reads "99+": the exact count is on the Progress page. */
  badgeMax: 99,
} as const;

/** One entry per area: a top-level route or a group of them (spec 005 "IPC / UI"); setup screens are reached through the guard instead. */
export const NAV: readonly NavEntry[] = [
  { kind: "link", to: "/", labelKey: "common.nav.home", icon: House },
  {
    kind: "group",
    pathPrefix: "/label",
    labelKey: "common.nav.labelling",
    icon: Tags,
    badge: "sessionPending",
    requires: "patterns",
    items: [
      { kind: "link", to: "/label", labelKey: "common.nav.labelling", icon: Tags },
      { kind: "link", to: "/label/progress", labelKey: "common.nav.progress", icon: ChartColumnBig, badge: "sessionPending" },
    ],
  },
  { kind: "link", to: "/skill", labelKey: "common.nav.skill", icon: Radar },
  { kind: "link", to: "/recommendations", labelKey: "common.nav.recommendations", icon: Compass },
  { kind: "link", to: "/settings", labelKey: "common.nav.settings", icon: Settings },
  { kind: "link", to: "/settings/identity", labelKey: "common.nav.identity", icon: UserRound },
];
