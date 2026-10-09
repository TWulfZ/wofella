import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { FlagToggles } from "./FlagToggles";

const FLAGS = { mixed: false, unsure: false, thumb: null };

function names(): (string | null)[] {
  return within(screen.getByRole("group", { name: "Flags" }))
    .getAllByRole("button")
    .map((b) => b.textContent);
}

describe("FlagToggles", () => {
  it("offers the thumb sides when the layout has a thumb column", () => {
    render(<FlagToggles flags={FLAGS} disabled={false} thumbs onToggle={() => undefined} />);
    expect(names()).toEqual(["Mixed", "Unsure", "Left thumb", "Right thumb"]);
  });

  it("drops the thumb sides on a layout without a thumb", () => {
    render(<FlagToggles flags={FLAGS} disabled={false} thumbs={false} onToggle={() => undefined} />);
    expect(names()).toEqual(["Mixed", "Unsure"]);
  });
});
