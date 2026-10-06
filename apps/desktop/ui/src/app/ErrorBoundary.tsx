import { useRouter, type ErrorComponentProps } from "@tanstack/react-router";
import { Component, type ErrorInfo, type ReactNode } from "react";
import { CircleX, RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toIpcError } from "@/ipc/client";
import { useErrorText } from "@/ipc/errorText";
import { BrandMark } from "@/shared/ui/brand-mark";
import { Button } from "@/shared/ui/button";

interface ErrorViewProps {
  error: unknown;
  onRetry?: () => void;
}

export function ErrorView({ error, onRetry }: ErrorViewProps) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const ipcError = toIpcError(error);
  return (
    <div className="flex justify-center px-6 py-12">
      <div
        role="alert"
        className="bg-card ring-destructive/40 flex w-full max-w-xl flex-col items-center gap-3 rounded-xl p-8 text-center shadow-lg shadow-black/20 ring-1"
      >
        <div className="bg-destructive/15 grid size-12 place-items-center rounded-full">
          <CircleX className="text-destructive size-6" aria-hidden="true" />
        </div>
        <h2 className="font-display text-xl font-bold">{t("error.title")}</h2>
        <p className="text-muted-foreground">{errorText(ipcError)}</p>
        {ipcError.details !== null && (
          <details className="text-muted-foreground self-stretch text-left text-xs">
            <summary className="cursor-pointer">{t("error.details")}</summary>
            <code className="bg-muted mt-2 block rounded-md px-3 py-2 font-mono break-all">
              {ipcError.code}: {ipcError.details}
            </code>
          </details>
        )}
        {ipcError.retryable && onRetry !== undefined && (
          <Button className="mt-2" onClick={onRetry}>
            <RotateCcw aria-hidden="true" />
            {t("common.retry")}
          </Button>
        )}
      </div>
    </div>
  );
}

export function RouteErrorView({ error, reset }: ErrorComponentProps) {
  const router = useRouter();
  return (
    <ErrorView
      error={error}
      onRetry={() => {
        reset();
        void router.invalidate();
      }}
    />
  );
}

// The root route's own error replaces RootLayout, so it brings a minimal shell (no nav: that needs the root context).
// Child-route errors render inside RootLayout and keep using RouteErrorView, or they would get two headers.
export function RootErrorView(props: ErrorComponentProps) {
  const { t } = useTranslation();
  return (
    <div className="bg-background text-foreground flex min-h-screen flex-col">
      <header className="bg-header osu-triangles h-14 border-b">
        <div className="mx-auto flex h-full w-full max-w-5xl items-center gap-2.5 px-6">
          <BrandMark className="size-8" />
          <span className="font-display text-lg font-bold tracking-tight">{t("common.appName")}</span>
        </div>
      </header>
      <main className="flex flex-1 flex-col justify-center">
        <RouteErrorView {...props} />
      </main>
    </div>
  );
}

interface ErrorBoundaryState {
  error: unknown;
  hasError: boolean;
}

// Catches render errors outside the router (providers, the router itself); route errors use RouteErrorView.
export class ErrorBoundary extends Component<{ children: ReactNode }, ErrorBoundaryState> {
  override state: ErrorBoundaryState = { error: null, hasError: false };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    return { error, hasError: true };
  }

  override componentDidCatch(error: unknown, info: ErrorInfo): void {
    console.error("render error", error, info.componentStack);
  }

  override render(): ReactNode {
    if (this.state.hasError) {
      return (
        <ErrorView
          error={this.state.error}
          onRetry={() => {
            this.setState({ error: null, hasError: false });
          }}
        />
      );
    }
    return this.props.children;
  }
}
