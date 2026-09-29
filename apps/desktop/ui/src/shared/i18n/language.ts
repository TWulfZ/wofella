export const SUPPORTED_LANGUAGES = ["en", "es"] as const;
export type Language = (typeof SUPPORTED_LANGUAGES)[number];

export const FALLBACK_LANGUAGE: Language = "en";
export const LANGUAGE_STORAGE_KEY = "wolluf.lang";

export function isLanguage(value: unknown): value is Language {
  return typeof value === "string" && (SUPPORTED_LANGUAGES as readonly string[]).includes(value);
}

// Spec 005 "Language": a stored choice wins, otherwise es* → es and everything else → en.
export function resolveInitialLanguage(stored: string | null, navigatorLanguage: string | undefined): Language {
  if (isLanguage(stored)) {
    return stored;
  }
  return navigatorLanguage?.toLowerCase().startsWith("es") === true ? "es" : FALLBACK_LANGUAGE;
}

// localStorage can be missing or throw (private mode, blocked storage); the UI must still start.
export function readStoredLanguage(): string | null {
  try {
    return window.localStorage.getItem(LANGUAGE_STORAGE_KEY);
  } catch {
    return null;
  }
}
