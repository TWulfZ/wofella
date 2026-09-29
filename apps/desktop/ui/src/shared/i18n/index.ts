import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { FALLBACK_LANGUAGE, readStoredLanguage, resolveInitialLanguage, SUPPORTED_LANGUAGES } from "./language";
import { resources } from "./resources";

export { i18n };
export {
  isLanguage,
  LANGUAGE_STORAGE_KEY,
  type Language,
  resolveInitialLanguage,
  SUPPORTED_LANGUAGES,
} from "./language";

export async function initI18n(): Promise<typeof i18n> {
  await i18n.use(initReactI18next).init({
    resources,
    lng: resolveInitialLanguage(readStoredLanguage(), navigator.language),
    fallbackLng: FALLBACK_LANGUAGE,
    supportedLngs: SUPPORTED_LANGUAGES,
    ns: ["translation"],
    defaultNS: "translation",
    // React already escapes rendered strings; escaping twice would show entities in the UI.
    interpolation: { escapeValue: false },
    returnNull: false,
  });
  return i18n;
}
