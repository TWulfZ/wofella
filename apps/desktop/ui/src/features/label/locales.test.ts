import { describe, expect, it } from "vitest";
import { localeFiles } from "@/shared/i18n/resources";

// The labeling service emits these as message keys, so a missing one would show the raw key.
const SERVICE_ERROR_KEYS = ["unknown_pattern"] as const;

describe("label locale files", () => {
  it.each(["en", "es"] as const)("word the labeling service errors in %s", (language) => {
    const file = localeFiles.find((f) => f.domain === "label" && f.language === language);
    const errors = (file?.content["label"] as { error?: Record<string, unknown> } | undefined)?.error ?? {};
    for (const key of SERVICE_ERROR_KEYS) {
      expect(errors[key], `label.error.${key}`).toEqual(expect.stringMatching(/\S/));
    }
  });
});
