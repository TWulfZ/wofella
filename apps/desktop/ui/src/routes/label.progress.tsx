import { createFileRoute } from "@tanstack/react-router";
import { PatternsUnavailable } from "@/features/label";
import { LabelProgressPage } from "@/features/labelProgress";
import { DEFAULT_KEYMODE, keymodesQuery, patternAvailability } from "@/features/players";

export const Route = createFileRoute("/label/progress")({
  loaderDeps: ({ search }) => ({ keymode: search.keymode ?? DEFAULT_KEYMODE }),
  // A failed read means "unknown", which keeps the page as before rather than hiding it.
  loader: async ({ context, deps }) =>
    patternAvailability(await context.queryClient.query(keymodesQuery()).catch(() => undefined), deps.keymode),
  component: ProgressPage,
});

function ProgressPage() {
  const { keymode } = Route.useLoaderDeps();
  const { patterns, alternatives } = Route.useLoaderData();
  if (!patterns) {
    return <PatternsUnavailable keymode={keymode} alternatives={alternatives} />;
  }
  return <LabelProgressPage keymode={keymode} />;
}
