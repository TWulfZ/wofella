import { describe, expect, it } from "vitest";
import { validateGlobalSearch } from "./search";

describe("validateGlobalSearch", () => {
  it("keeps valid scope, keymode and merge values", () => {
    expect(validateGlobalSearch({ scope: "self", keymode: 7, merge: "separate" })).toEqual({
      scope: "self",
      keymode: 7,
      merge: "separate",
    });
    expect(validateGlobalSearch({ scope: "all" })).toEqual({ scope: "all" });
    expect(validateGlobalSearch({ scope: "p:12", keymode: "4" })).toEqual({ scope: "p:12", keymode: 4 });
  });

  it("drops invalid values instead of failing the route", () => {
    expect(
      validateGlobalSearch({ scope: "p:", keymode: 0, merge: "sideways", other: "kept?" }),
    ).toEqual({});
    expect(validateGlobalSearch({ scope: "p:01" })).toEqual({});
    expect(validateGlobalSearch({ scope: "p:-3", keymode: 17 })).toEqual({});
    expect(validateGlobalSearch({ scope: 5, keymode: 7.5 })).toEqual({});
  });
});
