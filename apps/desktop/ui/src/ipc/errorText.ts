import { useTranslation } from "react-i18next";
import { fallbackMessageKey, toIpcError, type IpcError } from "./client";

interface Translator {
  t: (key: string, options?: Record<string, string>) => string;
  exists: (key: string) => boolean;
}

// §7: the UI renders only message keys, never Rust prose; a key the slice has not translated yet falls back to the
// per-code text so the user still gets a meaningful message.
export function localizeIpcError(error: IpcError, { t, exists }: Translator): string {
  const key = exists(error.messageKey) ? error.messageKey : fallbackMessageKey(error.code);
  return t(key, error.args);
}

/** Maps any thrown value (IpcFailure or not) to its localized message in the current language. */
export function useErrorText(): (error: unknown) => string {
  const { t, i18n } = useTranslation();
  return (error) =>
    localizeIpcError(toIpcError(error), {
      t: (key, options) => t(key, options ?? {}),
      exists: (key) => i18n.exists(key),
    });
}
