import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { IpcFailure, type IpcError } from "@/ipc/client";
import { i18n } from "@/shared/i18n";
import { ErrorBoundary, ErrorView } from "./ErrorBoundary";

function failure(overrides: Partial<IpcError> & Pick<IpcError, "code">): IpcFailure {
  return new IpcFailure({
    messageKey: `error.code.${overrides.code}`,
    args: {},
    details: null,
    retryable: false,
    ...overrides,
  });
}

describe("ErrorView", () => {
  it("renders the message key with its args", () => {
    render(<ErrorView error={failure({ code: "OSU_DIR_NOT_FOUND", args: { path: "/mnt/c/nowhere" } })} />);
    expect(screen.getByRole("alert")).toHaveTextContent("No valid osu! stable install at “/mnt/c/nowhere”.");
  });

  it("uses an emitting slice's own key when it exists", () => {
    render(
      <ErrorView error={failure({ code: "CONFLICT", messageKey: "error.instance_running", args: { path: "/d" } })} />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("wolluf is already running with this data folder (/d).");
  });

  it("falls back to error.code.<CODE> when the message key is missing", () => {
    render(<ErrorView error={failure({ code: "NOT_FOUND", messageKey: "plays.error.not_translated_yet" })} />);
    expect(screen.getByRole("alert")).toHaveTextContent("The requested item was not found.");
    expect(screen.queryByText(/not_translated_yet/)).not.toBeInTheDocument();
  });

  it("localizes in Spanish", async () => {
    await i18n.changeLanguage("es");
    render(<ErrorView error={failure({ code: "CANCELLED" })} />);
    expect(screen.getByRole("alert")).toHaveTextContent("La operación se canceló.");
  });

  it("shows Retry only for retryable errors", async () => {
    const onRetry = vi.fn();
    const { rerender } = render(<ErrorView error={failure({ code: "OSU_RUNNING" })} onRetry={onRetry} />);
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();

    rerender(<ErrorView error={failure({ code: "OSU_RUNNING", retryable: true })} onRetry={onRetry} />);
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it("renders a non-IPC error as INTERNAL and keeps raw prose out of the message", () => {
    render(<ErrorView error={new Error("database is locked at /home/someone")} />);
    const message = screen.getByText(/Unexpected internal error/);
    expect(message).not.toHaveTextContent("/home/someone");
    // The raw value survives only in the collapsed technical details, for bug reports.
    expect(screen.getByText(/INTERNAL: Error: database is locked/)).toBeInTheDocument();
  });
});

describe("ErrorBoundary", () => {
  it("catches a render error and shows the localized fallback", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    function Thrower(): never {
      throw failure({ code: "PARSE_FAILED" });
    }
    render(
      <ErrorBoundary>
        <Thrower />
      </ErrorBoundary>,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("A file could not be read; it may be damaged.");
  });
});
