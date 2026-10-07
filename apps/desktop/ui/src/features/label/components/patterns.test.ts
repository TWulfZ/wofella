import { describe, expect, it } from "vitest";
import type { PatternDefDto } from "@/ipc/bindings";
import { buildSearchVocabulary, matchesSearch, parseSearch } from "./patterns";

const TAXONOMY: PatternDefDto[] = [
  { id: "7k.regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "exactly two notes in one column" },
  { id: "7k.regular.tech.split", axis: "7k.regular.tech", key: "sp", description: "hands split across the board" },
  { id: "7k.regular.speed.runningman", axis: "7k.regular.speed", key: "rm", description: "an anchor under a run" },
  { id: "7k.regular.stream.jumpstream", axis: "7k.regular.stream", key: "js", description: "stream with chords" },
  { id: "7k.ln.general.longnote", axis: "7k.ln.general", key: "gl", description: "held notes" },
  { id: "7k.ln.tech.ln_jumptrill", axis: "7k.ln.tech", key: "lt", description: "held trills" },
  { id: "7k.ln.inverse.full_inverse", axis: "7k.ln.inverse", key: "fi", description: "gaps where notes would be" },
  { id: "7k.ln.inverse.half_inverse", axis: "7k.ln.inverse", key: "hi", description: "half the gaps" },
  { id: "7k.ln.release.shield", axis: "7k.ln.release", key: "sh", description: "release then press" },
];

// The label/locales axis and family labels (with the family aliases), in English and Spanish.
const AXIS_LABELS: Record<string, readonly string[]> = {
  regular_jack: ["Jack", "Jack"],
  regular_tech: ["Tech", "Tech"],
  regular_speed: ["Speed", "Velocidad"],
  regular_stream: ["Stream", "Stream"],
  ln_general: ["LN general", "LN general"],
  ln_tech: ["LN tech", "LN tech"],
  ln_inverse: ["LN inverse", "LN inverso"],
  ln_release: ["LN release", "LN release"],
};
const FAMILY_LABELS: Record<string, readonly string[]> = {
  regular: ["RICE", "RICE"],
  ln: ["LN", "LN", "long note", "long notes", "nota larga", "notas largas"],
};

const VOCABULARY = buildSearchVocabulary(
  TAXONOMY,
  (key) => AXIS_LABELS[key] ?? [],
  (family) => FAMILY_LABELS[family] ?? [],
);

function search(query: string): string[] {
  const tokens = parseSearch(query, VOCABULARY);
  return TAXONOMY.filter((pattern) => matchesSearch(pattern, tokens)).map((pattern) => pattern.key);
}

describe("pattern search", () => {
  it("matches everything for an empty or blank query", () => {
    expect(search("")).toHaveLength(TAXONOMY.length);
    expect(search("   ")).toHaveLength(TAXONOMY.length);
  });

  it.each([
    ["a name, ignoring case", "JUMPSTREAM", ["js"]],
    ["a key", "rm", ["rm"]],
    ["a description", "exactly two", ["mj"]],
  ])("still matches %s", (_, query, expected) => {
    expect(search(query)).toEqual(expected);
  });

  it("needs every word to match", () => {
    expect(search("held trills")).toEqual(["lt"]);
    expect(search("held zzz")).toEqual([]);
  });

  it("restricts to a family named by a word", () => {
    expect(search("ln")).toEqual(["gl", "lt", "fi", "hi", "sh"]);
    expect(search("RICE")).toEqual(["mj", "sp", "rm", "js"]);
    expect(search("regular")).toEqual(["mj", "sp", "rm", "js"]);
  });

  it.each(["long note", "long notes", "notas largas", "nota larga"])("reads “%s” as the LN family", (query) => {
    expect(search(query)).toEqual(["gl", "lt", "fi", "hi", "sh"]);
  });

  it("restricts to the axis a word names, in either language", () => {
    expect(search("ln inverse")).toEqual(["fi", "hi"]);
    expect(search("inverso")).toEqual(["fi", "hi"]);
    expect(search("velocidad")).toEqual(["rm"]);
    expect(search("speed")).toEqual(["rm"]);
  });

  it("keeps a family and an axis apart: rice tech is only RICE's tech axis", () => {
    expect(search("rice tech")).toEqual(["sp"]);
    expect(search("tech rice")).toEqual(["sp"]);
    expect(search("ln tech")).toEqual(["lt"]);
  });

  it("matches every axis of that name across families when no family is named", () => {
    expect(search("tech")).toEqual(["sp", "lt"]);
  });

  it("finds nothing for a family and an axis that never meet", () => {
    expect(search("ln jack")).toEqual([]);
    expect(search("rice inverse")).toEqual([]);
  });

  it("narrows a family or axis by the remaining words", () => {
    expect(search("ln half")).toEqual(["hi"]);
    expect(search("inverse full")).toEqual(["fi"]);
  });

  it("ignores accents and spacing", () => {
    expect(search("  LN    INVERSO ")).toEqual(["fi", "hi"]);
    expect(search("nótas largas")).toEqual(["gl", "lt", "fi", "hi", "sh"]);
  });
});

// Entries copied verbatim from crates/engine/src/taxonomy.rs whose descriptions use a family or axis word.
const REAL_TAXONOMY: PatternDefDto[] = [
  { id: "7k.regular.jack.anchor", axis: "7k.regular.jack", key: "a", description: "one column recurring every other row under a stream" },
  { id: "7k.regular.stream.jumpstream", axis: "7k.regular.stream", key: "js", description: "stream with two-note chords mixed in" },
  { id: "7k.regular.tech.split", axis: "7k.regular.tech", key: "sp", description: "hands split across the board" },
  { id: "7k.ln.general.chord", axis: "7k.ln.general", key: "lc", description: "long-note chords pressed or released together" },
  { id: "7k.ln.tech.hybrid", axis: "7k.ln.tech", key: "lh", description: "long notes held while tapping rice notes" },
  { id: "7k.ln.release.timing", axis: "7k.ln.release", key: "lr", description: "release timing: tails that need precise, staggered releases" },
];

function searchReal(query: string): string[] {
  const vocabulary = buildSearchVocabulary(
    REAL_TAXONOMY,
    (key) => AXIS_LABELS[key] ?? [],
    (family) => FAMILY_LABELS[family] ?? [],
  );
  const tokens = parseSearch(query, vocabulary);
  return REAL_TAXONOMY.filter((pattern) => matchesSearch(pattern, tokens)).map((pattern) => pattern.key);
}

// Deliberate: a family or axis word is a filter, never free text, so "rice tech" cannot leak LN tech. The cost is that
// a description using the word no longer matches by it; those patterns are found by their own name.
describe("pattern search on the real taxonomy", () => {
  it("keeps rice to the RICE family, so the LN hybrid is found as hybrid", () => {
    expect(searchReal("rice")).toEqual(["a", "js", "sp"]);
    expect(searchReal("ln rice")).toEqual([]);
    expect(searchReal("hybrid")).toEqual(["lh"]);
    expect(searchReal("ln hybrid")).toEqual(["lh"]);
  });

  it("keeps an axis word to its axis, so a jack described under a stream is not a stream", () => {
    expect(searchReal("stream")).toEqual(["js"]);
    expect(searchReal("release")).toEqual(["lr"]);
    expect(searchReal("anchor")).toEqual(["a"]);
  });
});
