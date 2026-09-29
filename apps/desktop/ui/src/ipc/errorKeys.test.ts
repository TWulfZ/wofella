import { describe, expect, it } from "vitest";
import { localeFiles } from "@/shared/i18n/resources";
import { SUPPORTED_LANGUAGES } from "@/shared/i18n";
import { ERROR_CODES } from "./client";

// UI-side twin of the Rust `i18n_error_keys` test: every wire code needs a fallback in every language, so an
// IpcError can always be rendered even when its slice key is missing.
function lookup(content: Record<string, unknown>, path: string[]): unknown {
  return path.reduce<unknown>(
    (node, key) => (node !== null && typeof node === "object" ? (node as Record<string, unknown>)[key] : undefined),
    content,
  );
}

// Keys emitted by Rust outside the per-code fallbacks (003 context and store errors); slices own theirs in their tests.
const EMITTED_ERROR_KEYS = ["error.instance_running", "error.data_dir_inside_osu"];

describe("error.json", () => {
  it.each(SUPPORTED_LANGUAGES)("%s has a fallback for every ErrorCode", (lng) => {
    const file = localeFiles.find((f) => f.language === lng && f.domain === "error");
    expect(file).toBeDefined();
    for (const code of ERROR_CODES) {
      expect(typeof lookup(file?.content ?? {}, ["error", "code", code]), code).toBe("string");
    }
  });

  it.each(SUPPORTED_LANGUAGES)("%s has the context keys emitted by the app", (lng) => {
    const file = localeFiles.find((f) => f.language === lng && f.domain === "error");
    for (const key of EMITTED_ERROR_KEYS) {
      expect(typeof lookup(file?.content ?? {}, key.split(".")), key).toBe("string");
    }
  });
});
