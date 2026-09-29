import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderControls } from "./scopeControls.testkit";

describe("NotSelfBanner", () => {
  it("is absent for the self profile", async () => {
    renderControls("/?scope=self");
    await screen.findByRole("combobox", { name: "Viewing" });
    expect(screen.queryByRole("note")).not.toBeInTheDocument();
  });

  it("shows for another player's profile", async () => {
    renderControls("/?scope=p:2");
    expect(await screen.findByRole("note")).toHaveTextContent(
      "Viewing Rosalind: not your profile. Nothing here affects your skill.",
    );
  });

  it("shows for All players", async () => {
    renderControls("/?scope=all");
    expect(await screen.findByRole("note")).toHaveTextContent("All players: mixed, not a person");
  });
});
