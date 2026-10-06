import { describe, expect, it } from "vitest";
import { type Answer, parseAnswer, RESERVED_TOKENS } from "./answer";

function labels(tokens: string[], toggleMixed: boolean, toggleUnsure: boolean): Answer {
  return { kind: "labels", tokens, noPattern: false, toggleMixed, toggleUnsure, thumb: null };
}

describe("parseAnswer", () => {
  it("reads the no-pattern and thumb side tokens", () => {
    const none = (toggleMixed: boolean, thumb: "left" | "right" | null): Answer => ({
      kind: "labels",
      tokens: [],
      noPattern: true,
      toggleMixed,
      toggleUnsure: false,
      thumb,
    });
    expect(parseAnswer("x")).toEqual({ ok: true, answer: none(false, null) });
    expect(parseAnswer("x tl m")).toEqual({ ok: true, answer: none(true, "left") });
    expect(parseAnswer("js tr")).toEqual({
      ok: true,
      answer: { kind: "labels", tokens: ["js"], noPattern: false, toggleMixed: false, toggleUnsure: false, thumb: "right" },
    });
    expect(parseAnswer("tl")).toEqual({ ok: true, answer: { kind: "thumb", side: "left" } });
    expect(parseAnswer("TR")).toEqual({ ok: true, answer: { kind: "thumb", side: "right" } });
    expect(parseAnswer("x js")).toEqual({ ok: false, error: { code: "noPatternWithPatterns" } });
    expect(parseAnswer("js tl tr")).toEqual({ ok: false, error: { code: "twoThumbSides" } });
  });

  it("turns label lines into tokens and toggles", () => {
    expect(parseAnswer("js\n")).toEqual({ ok: true, answer: labels(["js"], false, false) });
    // Resolution and dedup belong to the app.
    expect(parseAnswer(" CJ,js , cj ")).toEqual({ ok: true, answer: labels(["cj", "js", "cj"], false, false) });
    expect(parseAnswer("a m ? m js")).toEqual({ ok: true, answer: labels(["a", "js"], false, true) });
    expect(parseAnswer("lj m")).toEqual({ ok: true, answer: labels(["lj"], true, false) });
  });

  it("takes commands only when they stand alone", () => {
    const cases: [string, Answer][] = [
      ["m", { kind: "mixed" }],
      ["?", { kind: "unsure" }],
      ["s", { kind: "skip" }],
      ["u", { kind: "undo" }],
      ["w+", { kind: "reshape", op: "widen" }],
      ["w-", { kind: "reshape", op: "narrow" }],
      ["n", { kind: "reshape", op: "next" }],
      ["p", { kind: "reshape", op: "prev" }],
      ["h", { kind: "help" }],
      ["Q\n", { kind: "leave" }],
    ];
    for (const [line, answer] of cases) {
      expect(parseAnswer(line), line).toEqual({ ok: true, answer });
    }
  });

  it("returns error codes for bad answers", () => {
    expect(parseAnswer("js s")).toEqual({ ok: false, error: { code: "commandInLine", word: "s" } });
    expect(parseAnswer("m ?")).toEqual({ ok: false, error: { code: "noPatternGiven" } });
    expect(parseAnswer("  \n")).toEqual({ ok: false, error: { code: "empty" } });
  });

  it("reserves every command and the no-pattern answer", () => {
    expect([...RESERVED_TOKENS].sort()).toEqual(["?", "h", "m", "n", "p", "q", "s", "tl", "tr", "u", "w+", "w-", "x"]);
  });
});
