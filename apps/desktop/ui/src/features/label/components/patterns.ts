import type { PatternDefDto } from "@/ipc/bindings";

export interface AxisGroup {
  axis: string;
  patterns: PatternDefDto[];
}

/** Taxonomy order is meaningful (the CLI legend follows it), so groups keep first-seen order. */
export function groupByAxis(taxonomy: readonly PatternDefDto[]): AxisGroup[] {
  const groups: AxisGroup[] = [];
  for (const pattern of taxonomy) {
    const group = groups.find((g) => g.axis === pattern.axis);
    if (group === undefined) {
      groups.push({ axis: pattern.axis, patterns: [pattern] });
    } else {
      group.patterns.push(pattern);
    }
  }
  return groups;
}

/** `7k.regular.jack` → `regular_jack`: the keymode prefix does not change the axis name. */
export function axisKey(axis: string): string {
  return axis.split(".").slice(1).join("_");
}

export function patternName(id: string): string {
  return (id.split(".").at(-1) ?? id).replaceAll("_", " ");
}

/** A word of the query: it names a family, names axes, or matches a pattern's own text. */
export type SearchToken =
  | { kind: "family"; family: string }
  | { kind: "axis"; axes: ReadonlySet<string> }
  | { kind: "text"; text: string };

interface VocabularyEntry {
  words: readonly string[];
  token: SearchToken;
}

/** Phrases that name a family or an axis, longest first so "long notes" wins over a shorter match. */
export interface SearchVocabulary {
  entries: readonly VocabularyEntry[];
}

// Community names for the families, the same in every language (ADR 0017: community terms win); localized aliases
// come in through `familyLabels`.
const FAMILY_WORDS: Readonly<Record<string, readonly string[]>> = {
  regular: ["rice", "regular"],
  ln: ["ln", "lns"],
};

function normalize(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

function words(text: string): string[] {
  return normalize(text).split(/\s+/).filter((word) => word !== "");
}

/**
 * `axisLabels` and `familyLabels` give the UI's labels (and family aliases) in every language, so a Spanish query works in the English UI
 * too. An axis label's family words ("LN inverse") are dropped: "ln" alone must name the family, not every LN axis.
 */
export function buildSearchVocabulary(
  taxonomy: readonly PatternDefDto[],
  axisLabels: (axisKey: string) => readonly string[],
  familyLabels: (family: string) => readonly string[],
): SearchVocabulary {
  const families = new Map<string, string>();
  const axes = new Map<string, Set<string>>();
  const axisIds = [...new Set(taxonomy.map((pattern) => pattern.axis))];
  for (const family of new Set(axisIds.map(axisFamily))) {
    for (const phrase of [...(FAMILY_WORDS[family] ?? []), family, ...familyLabels(family)]) {
      const key = words(phrase).join(" ");
      if (key !== "") {
        families.set(key, family);
      }
    }
  }
  const familyWords = new Set(families.keys());
  for (const axis of axisIds) {
    const tail = axis.split(".").at(-1) ?? axis;
    for (const label of [tail, ...axisLabels(axisKey(axis))]) {
      const key = words(label)
        .filter((word) => !familyWords.has(word))
        .join(" ");
      if (key !== "" && !familyWords.has(key)) {
        axes.set(key, (axes.get(key) ?? new Set()).add(axis));
      }
    }
  }
  const entries: VocabularyEntry[] = [
    ...[...families].map(([key, family]) => ({ words: key.split(" "), token: { kind: "family", family } as const })),
    ...[...axes].map(([key, ids]) => ({ words: key.split(" "), token: { kind: "axis", axes: ids } as const })),
  ];
  entries.sort((a, b) => b.words.length - a.words.length);
  return { entries };
}

/** Splits the query into words; each word, or phrase, that the vocabulary knows becomes a family or axis filter. */
export function parseSearch(query: string, vocabulary: SearchVocabulary): SearchToken[] {
  const all = words(query);
  const tokens: SearchToken[] = [];
  let i = 0;
  while (i < all.length) {
    const entry = vocabulary.entries.find((e) => e.words.every((word, k) => all[i + k] === word));
    if (entry === undefined) {
      tokens.push({ kind: "text", text: all[i] ?? "" });
      i += 1;
    } else {
      tokens.push(entry.token);
      i += entry.words.length;
    }
  }
  return tokens;
}

/** Every token must match; no tokens match everything. */
export function matchesSearch(pattern: PatternDefDto, tokens: readonly SearchToken[]): boolean {
  const fields = [patternName(pattern.id), pattern.key, pattern.description].map(normalize);
  return tokens.every((token) => {
    switch (token.kind) {
      case "family":
        return axisFamily(pattern.axis) === token.family;
      case "axis":
        return token.axes.has(pattern.axis);
      case "text":
        return fields.some((field) => field.includes(token.text));
    }
  });
}

export interface FamilyGroup {
  /** `regular` (RICE) or `ln` for 7K; any other family keeps its own section. */
  family: string;
  axes: AxisGroup[];
}

/** `7k.ln.release` → `ln`: RICE and LN are master sections whatever the keymode. */
export function axisFamily(axis: string): string {
  return axis.split(".")[1] ?? axis;
}

export function groupByFamily(groups: readonly AxisGroup[]): FamilyGroup[] {
  const families: FamilyGroup[] = [];
  for (const group of groups) {
    const family = axisFamily(group.axis);
    const existing = families.find((f) => f.family === family);
    if (existing === undefined) {
      families.push({ family, axes: [group] });
    } else {
      existing.axes.push(group);
    }
  }
  return families;
}
