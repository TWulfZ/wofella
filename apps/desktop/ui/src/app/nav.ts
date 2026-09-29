import type { LinkProps } from "@tanstack/react-router";

export interface NavEntry {
  to: NonNullable<LinkProps["to"]>;
  labelKey: string;
}

/** One entry per top-level route (spec 005 "IPC / UI"); setup screens are reached through the guard instead. */
export const NAV: readonly NavEntry[] = [
  { to: "/", labelKey: "common.nav.home" },
  { to: "/settings", labelKey: "common.nav.settings" },
  { to: "/settings/identity", labelKey: "common.nav.identity" },
];
