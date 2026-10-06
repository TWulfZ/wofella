import { createRootRouteWithContext, Link, Outlet, redirect, useLocation } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { NAV } from "@/app/nav";
import { JobTray } from "@/features/jobs";
import { MergeCompareToggle, NotSelfBanner, ScopePicker, validateGlobalSearch } from "@/features/players";
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
});

function RootLayout() {
  const { t } = useTranslation();
  // During first-run setup there is no identity to scope by yet.
  const inSetup = useLocation({ select: (l) => l.pathname.startsWith(SETUP_PATH) });
  return (
    <div className="flex min-h-screen flex-col bg-background text-foreground">
      <header className="bg-header osu-triangles sticky top-0 z-20 flex h-14 items-center gap-6 border-b px-6">
        <div className="flex items-center gap-2.5">
          <BrandMark className="size-8" />
          <span className="font-display text-lg font-bold tracking-tight">{t("common.appName")}</span>
        </div>
        <nav aria-label={t("common.nav.label")} className="flex h-full items-center gap-1">
          {NAV.map(({ to, labelKey, icon: Icon }) => (
            <Link
              key={to}
              to={to}
              search={(prev) => prev}
              activeOptions={{ exact: true, includeSearch: false }}
              className="text-muted-foreground hover:text-foreground focus-visible:ring-ring data-[status=active]:text-foreground data-[status=active]:after:bg-primary relative inline-flex h-14 items-center gap-2 rounded-sm px-3 text-sm font-medium transition-colors duration-200 after:absolute after:inset-x-2 after:bottom-0 after:hidden after:h-[3px] after:rounded-full focus-visible:ring-2 focus-visible:outline-none data-[status=active]:after:block"
            >
              <Icon className="size-4" aria-hidden="true" />
              {t(labelKey)}
            </Link>
          ))}
        </nav>
        {!inSetup && (
          <div className="ml-auto flex items-center gap-3">
            <ScopePicker />
            <MergeCompareToggle />
          </div>
        )}
      </header>
      {!inSetup && <NotSelfBanner />}
      <main className="flex-1">
        <Outlet />
      </main>
      <JobTray />
    </div>
  );
}
