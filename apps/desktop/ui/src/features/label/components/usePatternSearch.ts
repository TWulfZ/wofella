import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { PatternDefDto } from "@/ipc/bindings";
import { SUPPORTED_LANGUAGES } from "@/shared/i18n/language";
import { buildSearchVocabulary, parseSearch, type SearchToken } from "./patterns";

/** Parses a pattern query with the family and axis names of every UI language, whichever one is shown. */
export function usePatternSearch(taxonomy: readonly PatternDefDto[]): (query: string) => SearchToken[] {
  const { i18n } = useTranslation();
  const vocabulary = useMemo(() => {
    const translators = SUPPORTED_LANGUAGES.map((language) => i18n.getFixedT(language));
    const labels = (key: string): string[] =>
      translators.map((t) => t(key, { defaultValue: "" })).filter((label) => label !== "");
    return buildSearchVocabulary(
      taxonomy,
      (axis) => labels(`label.axis.${axis}`),
      (family) => [
        ...labels(`label.patternGrid.family.${family}`),
        // A comma list, since locale values are strings only (i18n.test).
        ...labels(`label.patternGrid.familyAliases.${family}`).flatMap((aliases) => aliases.split(",")),
      ],
    );
  }, [taxonomy, i18n]);
  return (query) => parseSearch(query, vocabulary);
}
