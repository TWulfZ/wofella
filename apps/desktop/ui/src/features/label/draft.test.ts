import { describe, expect, it } from "vitest";
import { draftWords, isPatternActive, togglePattern, withoutThumb } from "./draft";

const JS = { id: "regular.stream.jumpstream", key: "js" };
const LJ = { id: "regular.jack.longjack", key: "lj" };

describe("draftWords", () => {
  it("splits like the answer grammar, lowercased", () => {
    expect(draftWords(" JS,lj  m ")).toEqual(["js", "lj", "m"]);
    expect(draftWords("")).toEqual([]);
  });
});

describe("isPatternActive", () => {
  it("matches the short key or the full id", () => {
    expect(isPatternActive("js m", JS)).toBe(true);
    expect(isPatternActive("regular.stream.jumpstream", JS)).toBe(true);
    expect(isPatternActive("jsx", JS)).toBe(false);
  });
});

describe("togglePattern", () => {
  it("appends the key when absent, keeping the rest of the line", () => {
    expect(togglePattern("", JS)).toBe("js");
    expect(togglePattern("lj m", JS)).toBe("lj m js");
  });

  it("drops a no-pattern answer when a pattern is picked", () => {
    expect(togglePattern("x m", JS)).toBe("m js");
  });

  it("removes every spelling of the pattern when present", () => {
    expect(togglePattern("js lj REGULAR.STREAM.JUMPSTREAM m", JS)).toBe("lj m");
    expect(togglePattern("lj", LJ)).toBe("");
  });
});

describe("withoutThumb", () => {
  it("drops every inline thumb side and keeps the rest of the line", () => {
    expect(withoutThumb("js TL m tr")).toBe("js m");
    expect(withoutThumb("lj")).toBe("lj");
    expect(withoutThumb("tl")).toBe("");
  });
});
