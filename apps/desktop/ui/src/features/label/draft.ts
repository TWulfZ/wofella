// The answer line is the single source of truth; chips only read and rewrite its words.

// Kept in step with answer.ts: picking a pattern contradicts a pending "no clear pattern" answer.
const NO_PATTERN = "x";
const THUMB_SIDES: ReadonlySet<string> = new Set(["tl", "tr"]);

export interface PatternRef {
  id: string;
  key: string;
}

export function draftWords(draft: string): string[] {
  return draft
    .toLowerCase()
    .split(/[\s,]+/)
    .filter((w) => w !== "");
}

function spells(word: string, pattern: PatternRef): boolean {
  return word === pattern.key.toLowerCase() || word === pattern.id.toLowerCase();
}

export function isPatternActive(draft: string, pattern: PatternRef): boolean {
  return draftWords(draft).some((w) => spells(w, pattern));
}

export function togglePattern(draft: string, pattern: PatternRef): string {
  const words = draftWords(draft);
  if (words.some((w) => spells(w, pattern))) {
    return words.filter((w) => !spells(w, pattern)).join(" ");
  }
  return [...words.filter((w) => w !== NO_PATTERN), pattern.key].join(" ");
}

export function withoutThumb(draft: string): string {
  return draftWords(draft)
    .filter((w) => !THUMB_SIDES.has(w))
    .join(" ");
}
