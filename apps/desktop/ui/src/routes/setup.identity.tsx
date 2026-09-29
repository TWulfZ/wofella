import { createFileRoute } from "@tanstack/react-router";
import { IdentityWizard } from "@/features/players";

export const Route = createFileRoute("/setup/identity")({
  component: () => <IdentityWizard mode="wizard" />,
});
