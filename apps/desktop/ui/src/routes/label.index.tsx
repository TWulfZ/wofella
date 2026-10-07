import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import {
  LabelScreen,
  labelPatternExamplesQuery,
  labelStatsQuery,
  labelTaxonomyQuery,
} from "@/features/label";
import { ProgressSummary, SessionMapStrip } from "@/features/labelProgress";
import { DEFAULT_KEYMODE } from "@/features/players";
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
  loader: ({ context, deps }) =>
    Promise.all([
      context.queryClient.query(labelTaxonomyQuery(deps.keymode)),
      context.queryClient.query(labelStatsQuery()),
      // Without examples the cards show placeholders, so a failure must not turn into the route's error page. They are
      // keyed by the preferred layout; a failed read means the profile default (null), as on the screen.
      context.queryClient
        .query(handLayoutQuery(deps.keymode))
        .catch(() => null)
        .then((layoutId) => context.queryClient.query(labelPatternExamplesQuery(deps.keymode, layoutId)))
        .catch(() => undefined),
    ]),
  component: LabelPage,
});

function LabelPage() {
  const { keymode } = Route.useLoaderDeps();
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
