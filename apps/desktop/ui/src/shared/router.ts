import type { QueryClient } from "@tanstack/react-query";

// Lives in shared so routes can type their context without depending on src/app (D14 matrix).
export interface RouterContext {
  queryClient: QueryClient;
}
