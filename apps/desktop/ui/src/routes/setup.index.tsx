import { createFileRoute } from "@tanstack/react-router";

// Placeholder so the first-run guard has a target; 005 T16 replaces it with the setup screen.
export const Route = createFileRoute("/setup/")({
  component: () => <div />,
});
