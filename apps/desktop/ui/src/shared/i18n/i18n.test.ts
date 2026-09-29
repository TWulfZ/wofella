import { describe, expect, it } from "vitest";
import { resolveInitialLanguage, SUPPORTED_LANGUAGES } from "./language";
import { localeFiles } from "./resources";

function flatten(value: unknown, prefix = ""): Map<string, unknown> {
  const out = new Map<string, unknown>();
  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    for (const [k, v] of Object.entries(value)) {
      for (const [fk, fv] of flatten(v, prefix === "" ? k : `${prefix}.${k}`)) {
        out.set(fk, fv);
      }
    }
  } else {
    out.set(prefix, value);
  }
  return out;
}

const domains = [...new Set(localeFiles.map((f) => f.domain))].sort();

describe("locale files", () => {
  it("exist for at least the common domain", () => {
    expect(domains).toContain("common");
  });

  it.each(domains)("%s exists in every supported language", (domain) => {
    const languages = localeFiles.filter((f) => f.domain === domain).map((f) => f.language);
    expect(languages.sort()).toEqual([...SUPPORTED_LANGUAGES].sort());
  });

  it.each(domains)("%s has identical key sets across languages", (domain) => {
    const keySets = SUPPORTED_LANGUAGES.map((lng) => {
      const file = localeFiles.find((f) => f.domain === domain && f.language === lng);
      return [...flatten(file?.content).keys()].sort();
    });
    for (const keys of keySets) {
      expect(keys).toEqual(keySets[0]);
    }
  });

  it.each(localeFiles.map((f) => [`${f.language}/${f.domain}`, f] as const))(
    "%s is namespaced under its domain and has only non-empty strings",
    (_, file) => {
      expect(Object.keys(file.content)).toEqual([file.domain]);
      for (const [key, value] of flatten(file.content)) {
        expect(typeof value, key).toBe("string");
        expect(value, key).not.toBe("");
      }
    },
  );
});

describe("resolveInitialLanguage", () => {
  it("prefers a stored supported choice", () => {
    expect(resolveInitialLanguage("es", "en-US")).toBe("es");
  });

  it("ignores an unsupported stored value", () => {
    expect(resolveInitialLanguage("fr", "es-AR")).toBe("es");
  });

  it("maps any es* navigator language to es", () => {
    expect(resolveInitialLanguage(null, "es-419")).toBe("es");
  });

  it("falls back to en for everything else", () => {
    expect(resolveInitialLanguage(null, "de-DE")).toBe("en");
    expect(resolveInitialLanguage(null, undefined)).toBe("en");
  });
});
