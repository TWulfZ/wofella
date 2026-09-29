import "@testing-library/jest-dom/vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// jsdom does not implement scrolling; the router's scroll restoration calls it on every navigation.
if (typeof window !== "undefined") {
  Object.defineProperty(window, "scrollTo", { value: () => undefined, writable: true });
}

afterEach(() => {
  cleanup();
  // Node-environment suites (the lint self-test) have no window for the Tauri mocks to reset.
  if (typeof window !== "undefined") {
    clearMocks();
  }
});
