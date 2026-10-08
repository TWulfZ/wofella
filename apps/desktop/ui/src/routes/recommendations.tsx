import { createFileRoute } from "@tanstack/react-router";
import { RecommendationsPage } from "@/features/preview";

export const Route = createFileRoute("/recommendations")({
  component: RecommendationsPage,
});
