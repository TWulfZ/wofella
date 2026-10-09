import { createFileRoute } from "@tanstack/react-router";
import { SkillPage } from "@/features/preview";

export const Route = createFileRoute("/skill")({
  component: SkillPage,
});
