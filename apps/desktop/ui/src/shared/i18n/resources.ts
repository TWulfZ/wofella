import type { Resource } from "i18next";
import { isLanguage, type Language } from "./language";

export interface LocaleFile {
  language: Language;
  domain: string;
  content: Record<string, unknown>;
}

// Per-domain files are discovered, so a slice adds translations by dropping in `<lng>/<domain>.json`.
const modules = import.meta.glob<Record<string, unknown>>("./locales/*/*.json", {
  eager: true,
  import: "default",
});

const LOCALE_PATH = /^\.\/locales\/([^/]+)\/([^/]+)\.json$/;

export const localeFiles: LocaleFile[] = Object.entries(modules).map(([path, content]) => {
  const match = LOCALE_PATH.exec(path);
  const language = match?.[1];
  const domain = match?.[2];
  if (!isLanguage(language) || domain === undefined) {
    throw new Error(`unexpected locale file ${path}`);
  }
  return { language, domain, content };
});

export const resources: Resource = localeFiles.reduce<Resource>((acc, file) => {
  const translation = acc[file.language]?.["translation"] ?? {};
  acc[file.language] = { translation: { ...(translation as object), ...file.content } };
  return acc;
}, {});
