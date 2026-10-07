import { createFileRoute } from "@tanstack/react-router";
import {
  LabelScreen,
  labelPatternExamplesQuery,
  labelStatsQuery,
  labelTaxonomyQuery,
} from "@/features/label";
import { DEFAULT_KEYMODE } from "@/features/players";
import { handLayoutQuery } from "@/features/preferences";

export const Route = createFileRoute("/label")({
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
  return <LabelScreen keymode={keymode} />;
}
