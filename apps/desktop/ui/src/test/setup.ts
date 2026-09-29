import "@testing-library/jest-dom/vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, vi } from "vitest";
import { i18n, initI18n } from "@/shared/i18n";

const hasWindow = typeof window !== "undefined";

// jsdom does not implement scrolling; the router's scroll restoration calls it on every navigation.
if (hasWindow) {
  Object.defineProperty(window, "scrollTo", { value: () => undefined, writable: true });
  // jsdom has no matchMedia; sonner's Toaster reads prefers-color-scheme through it on mount.
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string): MediaQueryList =>
      ({
        matches: false,
        media: query,
        onchange: null,
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
        addListener: () => undefined,
        removeListener: () => undefined,
        dispatchEvent: () => false,
      }) satisfies MediaQueryList,
  });
}

beforeAll(async () => {
  if (hasWindow && !i18n.isInitialized) {
    await initI18n();
  }
});

// Assertions are written against English strings; a test that switches language must not leak it.
beforeEach(async () => {
  if (hasWindow) {
    window.localStorage.clear();
    await i18n.changeLanguage("en");
  }
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  // Node-environment suites (the lint self-test) have no window for the Tauri mocks to reset.
  if (hasWindow) {
    clearMocks();
  }
});
