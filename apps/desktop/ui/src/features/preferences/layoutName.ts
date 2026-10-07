import { useTranslation } from "react-i18next";

/** The readable name of a layout preset id; an id the UI has no copy for yet is shown as-is. */
export function useLayoutName(): (id: string) => string {
  const { t, i18n } = useTranslation();
  return (id) => {
    // i18next splits keys on ".", and preset ids contain one.
    const key = `settings.handLayout.presets.${id.replaceAll(".", "_")}`;
    return i18n.exists(key) ? t(key) : id;
  };
}
