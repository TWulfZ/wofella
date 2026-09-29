import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { useEffect, type ReactNode } from "react";
import { jobTraySink } from "@/features/jobs";
import { startEventBridge, type JobEventSink } from "@/ipc/eventBridge";
import { Toaster } from "@/shared/ui/sonner";
import { TooltipProvider } from "@/shared/ui/tooltip";

function useEventBridge(queryClient: QueryClient, sink: JobEventSink): void {
  useEffect(() => {
    let stop: (() => void) | undefined;
    let disposed = false;
    startEventBridge(queryClient, sink)
      .then((unlisten) => {
        if (disposed) {
          unlisten();
        } else {
          stop = unlisten;
        }
      })
      .catch((e: unknown) => {
        console.error("event bridge failed to start", e);
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [queryClient, sink]);
}

interface AppProvidersProps {
  queryClient: QueryClient;
  children: ReactNode;
}

export function AppProviders({ queryClient, children }: AppProvidersProps) {
  useEventBridge(queryClient, jobTraySink);
  return (
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        {children}
        <Toaster />
      </TooltipProvider>
    </QueryClientProvider>
  );
}
