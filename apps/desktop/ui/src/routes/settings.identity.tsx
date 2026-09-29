import { createFileRoute } from "@tanstack/react-router";

// Placeholder for the settings link; 004 T12 replaces it with the alias table in edit mode.
export const Route = createFileRoute("/settings/identity")({
  component: () => <div />,
});
