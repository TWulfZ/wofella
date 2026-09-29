import { useRouter, type ErrorComponentProps } from "@tanstack/react-router";
import { Component, type ErrorInfo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { toIpcError } from "@/ipc/client";
import { useErrorText } from "@/ipc/errorText";
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
    <div role="alert" className="m-6 flex max-w-xl flex-col gap-3 rounded-lg border border-destructive/40 p-4">
      <h2 className="font-semibold">{t("error.title")}</h2>
      <p>{errorText(ipcError)}</p>
      {ipcError.details !== null && (
        <details className="text-muted-foreground text-xs">
          <summary>{t("error.details")}</summary>
          <code className="break-all">
            {ipcError.code}: {ipcError.details}
          </code>
        </details>
      )}
      {ipcError.retryable && onRetry !== undefined && (
        <Button className="self-start" onClick={onRetry}>
          {t("common.retry")}
        </Button>
      )}
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
