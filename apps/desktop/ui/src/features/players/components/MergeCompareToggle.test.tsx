import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ALL_PLAYERS, SELF_PROFILE } from "../fixtures";
import { renderControls } from "./scopeControls.testkit";

describe("MergeCompareToggle", () => {
  it("is hidden for a single-alias entry", async () => {
    renderControls("/?scope=p:2");
    await screen.findByRole("note");
    expect(screen.queryByRole("group", { name: "Alias view" })).not.toBeInTheDocument();
  });

  it("shows the persisted mode and writes a URL override", async () => {
    const { router } = renderControls("/?scope=self");
    const merged = await screen.findByRole("button", { name: "Merged" });
    expect(merged).toHaveAttribute("aria-pressed", "true");

    await userEvent.click(screen.getByRole("button", { name: "Compare" }));

    await waitFor(() => {
      expect(router.state.location.search).toMatchObject({ scope: "self", merge: "separate" });
    });
    expect(screen.getByRole("button", { name: "Compare" })).toHaveAttribute("aria-pressed", "true");
  });

  it("lets the URL merge param override the persisted mode", async () => {
    renderControls("/?scope=all&merge=separate");
    expect(await screen.findByRole("button", { name: "Compare" })).toHaveAttribute("aria-pressed", "true");
  });

  it("hides everything while a self profile has no names", async () => {
    renderControls("/", [{ ...SELF_PROFILE, aliasIds: [], scopes: [] }, ALL_PLAYERS]);
    expect(await screen.findByText("No names selected")).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Alias view" })).not.toBeInTheDocument();
  });
});
