import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { renderControls } from "./scopeControls.testkit";

describe("ScopePicker", () => {
  it("lists the profiles plus All players and falls back to the default profile", async () => {
    const { calls } = renderControls("/");
    const picker = await screen.findByRole("combobox", { name: "Viewing" });
    await waitFor(() => {
      expect(picker).toHaveValue("self");
    });
    expect(screen.getByRole("option", { name: "Me" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Rosalind" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "All players (mixed, not a person)" })).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "players_list_profiles")?.args).toEqual({ keymode: 7 });
  });

  it("writes the chosen entry into the URL", async () => {
    const { router } = renderControls("/");
    await userEvent.selectOptions(await screen.findByRole("combobox", { name: "Viewing" }), "all");
    await waitFor(() => {
      expect(router.state.location.search).toMatchObject({ scope: "all" });
    });
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Viewing" }), "p:2");
    await waitFor(() => {
      expect(router.state.location.search).toMatchObject({ scope: "p:2" });
    });
  });

  it("follows the URL keymode", async () => {
    const { calls } = renderControls("/?keymode=4");
    await screen.findByRole("combobox", { name: "Viewing" });
    expect(calls.find((c) => c.cmd === "players_list_profiles")?.args).toEqual({ keymode: 4 });
  });

  it("falls back to the default when the URL names a profile that does not exist", async () => {
    renderControls("/?scope=p:99");
    await waitFor(() => {
      expect(screen.getByRole("combobox", { name: "Viewing" })).toHaveValue("self");
    });
  });
});
