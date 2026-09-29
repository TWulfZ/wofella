import { createFileRoute } from "@tanstack/react-router";

// Placeholder so the first-run guard has a target; 004 T12 replaces it with the identity wizard.
export const Route = createFileRoute("/setup/identity")({
  component: () => <div />,
});
