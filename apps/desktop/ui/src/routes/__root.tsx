import { useQuery } from "@tanstack/react-query";
import { createRootRouteWithContext, Outlet, redirect, useLocation, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { RootErrorView } from "@/app/ErrorBoundary";
import { NAV } from "@/app/nav";
import { type NavBadgeCounts, NavGroupMenu, NavLinkItem, type NavUnmet } from "@/app/NavMenu";
import { JobTray } from "@/features/jobs";
import { usePendingSessionMaps } from "@/features/labelProgress";
import {
  DEFAULT_KEYMODE,
  keymodeHasPatterns,
  KeymodeSwitcher,
  keymodesQuery,
  MergeCompareToggle,
  NotSelfBanner,
  ScopePicker,
  validateGlobalSearch,
} from "@/features/players";
import { firstRunRedirect, SETUP_PATH, setupStatusQuery } from "@/features/setup";
import type { RouterContext } from "@/shared/router";
import { BrandMark } from "@/shared/ui/brand-mark";

export const Route = createRootRouteWithContext<RouterContext>()({
  // Scope, keymode and merge are global and live in the URL (§8); players owns their validation.
  validateSearch: validateGlobalSearch,
  // `query` refetches an invalidated status (DataChanged{setup}) before routing; the spec's ensureQueryData would
  // hand back the cached value and route on stale data.
  beforeLoad: async ({ context, location }) => {
    const status = await context.queryClient.query(setupStatusQuery());
    const target = firstRunRedirect(status, location.pathname);
    if (target !== null) {
      redirect({ to: target, replace: true, throw: true });
    }
  },
  component: RootLayout,
  errorComponent: RootErrorView,
});

function RootLayout() {
  const { t } = useTranslation();
  // During first-run setup there is no identity to scope by yet.
  const inSetup = useLocation({ select: (l) => l.pathname.startsWith(SETUP_PATH) });
  const keymode = useSearch({ strict: false, select: (search) => search.keymode ?? DEFAULT_KEYMODE });
  // Before setup there is no self profile, so nothing can be pending (ADR 0020).
  const badges: NavBadgeCounts = { sessionPending: usePendingSessionMaps(keymode, !inSetup) ?? 0 };
  const keymodes = useQuery(keymodesQuery());
  const unmet: NavUnmet = keymodeHasPatterns(keymodes.data, keymode)
    ? {}
    : { patterns: t("common.nav.labellingUnavailable", { keymode }) };
  return (
    <div className="flex min-h-screen flex-col bg-background text-foreground">
      <header className="bg-header osu-triangles sticky top-0 z-20 h-14 border-b">
        {/* Same column as PageHeader and the page content, so brand and controls share their edges. */}
        <div className="mx-auto flex h-full w-full max-w-5xl items-center gap-6 px-6">
          <div className="flex shrink-0 items-center gap-2.5">
            <BrandMark className="size-8" />
            <span className="font-display text-lg font-extrabold tracking-tight italic">{t("common.appName")}</span>
          </div>
          <nav aria-label={t("common.nav.label")} className="flex h-full shrink-0 items-center gap-1">
            {NAV.map((entry) =>
              entry.kind === "group" ? (
                <NavGroupMenu key={entry.pathPrefix} entry={entry} counts={badges} unmet={unmet} />
              ) : (
                <NavLinkItem key={entry.to} entry={entry} counts={badges} />
              ),
            )}
          </nav>
          {!inSetup && (
            <div className="ml-auto flex min-w-0 shrink items-center gap-3">
              <KeymodeSwitcher />
              <ScopePicker />
              <MergeCompareToggle />
            </div>
          )}
        </div>
      </header>
      {!inSetup && <NotSelfBanner />}
      <main className="flex flex-1 flex-col">
        <Outlet />
      </main>
      <JobTray />
    </div>
  );
}
