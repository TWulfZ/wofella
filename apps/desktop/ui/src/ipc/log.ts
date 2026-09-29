import { error as logError } from "@tauri-apps/plugin-log";
import { describeRaw } from "./client";

// Logging must never throw back into the page: a failing log call would re-enter unhandledrejection forever.
function forward(message: string, options?: { file: string; line: number }): void {
  logError(message, options).catch(() => undefined);
}

/** Sends uncaught UI errors to the Rust log file through plugin-log (spec 005 "Logging"); returns the uninstaller. */
export function installErrorForwarding(): () => void {
  const onError = (event: ErrorEvent) => {
    forward(`window.onerror: ${event.message}`, event.filename === "" ? undefined : { file: event.filename, line: event.lineno });
  };
  const onRejection = (event: PromiseRejectionEvent) => {
    forward(`unhandledrejection: ${describeRaw(event.reason)}`);
  };
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);
  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
  };
}
