import { QueryClient } from "@tanstack/react-query";

// Spec 005 "Query": invalidation is event-driven (DataChanged), and retryable errors are surfaced to the user
// with a Retry button instead of being retried silently.
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { staleTime: Infinity, retry: false },
      mutations: { retry: false },
    },
  });
}
