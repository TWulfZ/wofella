import { createRootRouteWithContext, Link, Outlet, redirect, useLocation } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { NAV } from "@/app/nav";
import { JobTray } from "@/features/jobs";
import { MergeCompareToggle, NotSelfBanner, ScopePicker, validateGlobalSearch } from "@/features/players";
import { firstRunRedirect, SETUP_PATH, setupStatusQuery } from "@/features/setup";
import type { RouterContext } from "@/shared/router";

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
      <header className="flex items-center gap-6 border-b px-6 py-3">
        <span className="font-semibold tracking-tight">{t("common.appName")}</span>
        <nav aria-label={t("common.nav.label")} className="flex gap-4 text-sm">
          {NAV.map((entry) => (
            <Link
              key={entry.to}
              to={entry.to}
              search={(prev) => prev}
              activeOptions={{ exact: true, includeSearch: false }}
              className="text-muted-foreground data-[status=active]:text-foreground data-[status=active]:font-medium"
            >
              {t(entry.labelKey)}
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
