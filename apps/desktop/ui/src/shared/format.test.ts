import { describe, expect, it } from "vitest";
import { formatRelative } from "./format";

const NOW = Date.parse("2026-10-07T12:00:00.000Z");

describe("formatRelative", () => {
  it.each([
    ["2026-10-07T11:59:30.000Z", "en", "30 seconds ago"],
    ["2026-10-07T11:55:00.000Z", "en", "5 minutes ago"],
    ["2026-10-07T09:00:00.000Z", "en", "3 hours ago"],
    ["2026-10-06T12:00:00.000Z", "en", "yesterday"],
    ["2026-10-02T12:00:00.000Z", "en", "5 days ago"],
    ["2026-10-07T09:00:00.000Z", "es", "hace 3 horas"],
  ])("words %s in %s as %s", (iso, language, expected) => {
    expect(formatRelative(iso, NOW, language)).toBe(expected);
  });
});
