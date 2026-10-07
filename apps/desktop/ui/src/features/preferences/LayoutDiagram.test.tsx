import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { K7_LAYOUTS } from "./layouts.testkit";
import { LayoutDiagram } from "./LayoutDiagram";

function layout(id: string) {
  const found = K7_LAYOUTS.find((l) => l.id === id);
  if (found === undefined) {
    throw new Error(id);
  }
  return found;
}

function draw(id: string) {
  const { container } = render(<LayoutDiagram columns={layout(id).columns} />);
  const svg = container.querySelector("svg");
  if (svg === null) {
    throw new Error("no svg");
  }
  return svg;
}

describe("LayoutDiagram", () => {
  it("is decorative: the option around it carries the name", () => {
    expect(draw("k7.313_right_thumb")).toHaveAttribute("aria-hidden", "true");
  });

  it("draws one key per column coloured by its hand", () => {
    const keys = [...draw("k7.both_thumbs").querySelectorAll("[data-column]")];
    expect(keys.map((k) => k.getAttribute("data-hand"))).toEqual(["left", "left", "left", "both", "right", "right", "right"]);
  });

  it.each([
    ["k7.313_right_thumb", ["3"]],
    ["k7.313_left_thumb", ["3"]],
    ["k7.43", []],
    ["k7.34", []],
    ["k7.both_thumbs", ["3"]],
  ])("marks the thumb column of %s", (id, thumbs) => {
    const marked = [...draw(id).querySelectorAll("[data-thumb]")].map((k) => k.getAttribute("data-thumb"));
    expect(marked).toEqual(thumbs);
  });

  it.each([
    ["k7.313_right_thumb", ["3"]],
    ["k7.313_left_thumb", ["4"]],
    ["k7.43", ["4"]],
    ["k7.34", ["3"]],
    ["k7.both_thumbs", ["3", "4"]],
  ])("draws the hand split of %s where the hand changes, as the playfield does", (id, splits) => {
    const lines = [...draw(id).querySelectorAll("[data-split]")].map((l) => l.getAttribute("data-split"));
    expect(lines).toEqual(splits);
  });
});
