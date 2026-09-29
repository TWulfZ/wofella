import { RouterProvider } from "@tanstack/react-router";
import { useState } from "react";
import { ErrorBoundary } from "./ErrorBoundary";
import { AppProviders } from "./providers";
import { createQueryClient } from "./queryClient";
import { createAppRouter } from "./router";

export function App() {
  const [queryClient] = useState(createQueryClient);
  const [router] = useState(() => createAppRouter(queryClient));
  return (
    <ErrorBoundary>
      <AppProviders queryClient={queryClient}>
        <RouterProvider router={router} />
      </AppProviders>
    </ErrorBoundary>
  );
}
