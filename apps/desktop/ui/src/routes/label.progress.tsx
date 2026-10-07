import { createFileRoute } from "@tanstack/react-router";
import { LabelProgressPage } from "@/features/labelProgress";
import { DEFAULT_KEYMODE } from "@/features/players";

export const Route = createFileRoute("/label/progress")({
  component: ProgressPage,
});

function ProgressPage() {
  const keymode = Route.useSearch({ select: (search) => search.keymode ?? DEFAULT_KEYMODE });
  return <LabelProgressPage keymode={keymode} />;
}
