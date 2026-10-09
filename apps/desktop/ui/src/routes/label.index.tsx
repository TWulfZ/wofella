import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import {
  LabelScreen,
  labelPatternExamplesQuery,
  labelStatsQuery,
  labelTaxonomyQuery,
  PatternsUnavailable,
} from "@/features/label";
import { ProgressSummary, SessionMapStrip } from "@/features/labelProgress";
import { DEFAULT_KEYMODE, keymodesQuery, patternAvailability } from "@/features/players";
import { handLayoutQuery } from "@/features/preferences";

const MD5 = /^[0-9a-f]{32}$/;

interface LabelSearch {
  /** A chart to open first, handed over by the session list; consumed on arrival. */
  chart?: string | undefined;
}

export const Route = createFileRoute("/label/")({
  validateSearch: (search: Record<string, unknown>): LabelSearch => ({
    chart: typeof search["chart"] === "string" && MD5.test(search["chart"]) ? search["chart"] : undefined,
  }),
  loaderDeps: ({ search }) => ({ keymode: search.keymode ?? DEFAULT_KEYMODE }),
  loader: async ({ context, deps }) => {
    // A keymode without a taxonomy has no label_taxonomy to load; a failed read means "unknown" and loads as before.
    const availability = patternAvailability(
      await context.queryClient.query(keymodesQuery()).catch(() => undefined),
      deps.keymode,
    );
    if (!availability.patterns) {
      return availability;
    }
    await Promise.all([
      context.queryClient.query(labelTaxonomyQuery(deps.keymode)),
      context.queryClient.query(labelStatsQuery()),
      // Without examples the cards show placeholders, so a failure must not turn into the route's error page. They are
      // keyed by the preferred layout; a failed read means the profile default (null), as on the screen.
      context.queryClient
        .query(handLayoutQuery(deps.keymode))
        .catch(() => null)
        .then((layoutId) => context.queryClient.query(labelPatternExamplesQuery(deps.keymode, layoutId)))
        .catch(() => undefined),
    ]);
    return availability;
  },
  component: LabelPage,
});

function LabelPage() {
  const { keymode } = Route.useLoaderDeps();
  const { patterns, alternatives } = Route.useLoaderData();
  if (!patterns) {
    return <PatternsUnavailable keymode={keymode} alternatives={alternatives} />;
  }
  return <LabelScreenPage keymode={keymode} />;
}

function LabelScreenPage({ keymode }: { keymode: number }) {
  const chart = Route.useSearch({ select: (search) => search.chart });
  const navigate = Route.useNavigate();
  // Held for the screen's life: the address drops it so nav links and a reload do not reopen the same chart.
  const [openChart] = useState(chart ?? null);
  useEffect(() => {
    if (chart !== undefined) {
      void navigate({ search: (prev) => ({ ...prev, chart: undefined }), replace: true });
    }
  }, [chart, navigate]);
  return (
    <LabelScreen
      keymode={keymode}
      openChart={openChart}
      countersDetails={<ProgressSummary keymode={keymode} />}
      sessionMap={(map) => <SessionMapStrip keymode={keymode} md5={map.md5} title={map.title} />}
    />
  );
}
