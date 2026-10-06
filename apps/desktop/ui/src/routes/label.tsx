import { createFileRoute } from "@tanstack/react-router";
import { LabelScreen, labelStatsQuery, labelTaxonomyQuery } from "@/features/label";
import { DEFAULT_KEYMODE } from "@/features/players";

export const Route = createFileRoute("/label")({
  loaderDeps: ({ search }) => ({ keymode: search.keymode ?? DEFAULT_KEYMODE }),
  loader: ({ context, deps }) =>
    Promise.all([
      context.queryClient.query(labelTaxonomyQuery(deps.keymode)),
      context.queryClient.query(labelStatsQuery()),
    ]),
  component: LabelPage,
});

function LabelPage() {
  const { keymode } = Route.useLoaderDeps();
  return <LabelScreen keymode={keymode} />;
}
