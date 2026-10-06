import type { LinkProps } from "@tanstack/react-router";
import { House, type LucideIcon, Settings, Tags, UserRound } from "lucide-react";

export interface NavEntry {
  to: NonNullable<LinkProps["to"]>;
  labelKey: string;
  icon: LucideIcon;
}

/** One entry per top-level route (spec 005 "IPC / UI"); setup screens are reached through the guard instead. */
export const NAV: readonly NavEntry[] = [
  { to: "/", labelKey: "common.nav.home", icon: House },
  { to: "/label", labelKey: "common.nav.labelling", icon: Tags },
  { to: "/settings", labelKey: "common.nav.settings", icon: Settings },
  { to: "/settings/identity", labelKey: "common.nav.identity", icon: UserRound },
];
