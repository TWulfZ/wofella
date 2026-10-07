import { createFileRoute } from "@tanstack/react-router";
import { LabelScreen, labelPatternExamplesQuery, labelStatsQuery, labelTaxonomyQuery } from "@/features/label";
import { DEFAULT_KEYMODE } from "@/features/players";

export const Route = createFileRoute("/label")({
  loaderDeps: ({ search }) => ({ keymode: search.keymode ?? DEFAULT_KEYMODE }),
  loader: ({ context, deps }) =>
    Promise.all([
      context.queryClient.query(labelTaxonomyQuery(deps.keymode)),
      context.queryClient.query(labelStatsQuery()),
      // Without examples the cards show placeholders, so a failure must not turn into the route's error page.
      context.queryClient.query(labelPatternExamplesQuery(deps.keymode)).catch(() => undefined),
    ]),
  component: LabelPage,
});

function LabelPage() {
  const { keymode } = Route.useLoaderDeps();
  return <LabelScreen keymode={keymode} />;
}
