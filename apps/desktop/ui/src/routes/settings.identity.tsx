import { createFileRoute } from "@tanstack/react-router";
import { IdentityWizard } from "@/features/players";

export const Route = createFileRoute("/settings/identity")({
  component: () => <IdentityWizard mode="settings" />,
});
