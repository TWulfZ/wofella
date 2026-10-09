import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { bootApp } from "@/app/testing";
import type { ProfileEntryDto } from "@/ipc/bindings";

const SELF: ProfileEntryDto = {
  ref: { kind: "profile", id: 1 },
  profileKind: "self",
  label: "Me",
  isDefault: true,
  mergeMode: "merged",
  aliasIds: [1, 2],
  scopes: [{ scopeHash: "a".repeat(64), aliasIds: [1, 2], keymode: 7 }],
};
const ALL: ProfileEntryDto = {
  ref: { kind: "all_players" },
  profileKind: "all_players",
  label: "",
  isDefault: false,
  mergeMode: "merged",
  aliasIds: [1, 2, 3],
  scopes: [{ scopeHash: "c".repeat(64), aliasIds: [1, 2, 3], keymode: 7 }],
};

function bootWithProfiles(path: string) {
  return bootApp(path, { playersListProfiles: () => [SELF, ALL] });
}

describe("root layout identity hosting", () => {
  it("mounts the ScopePicker and the Merged/Compare toggle in the header", async () => {
    await bootWithProfiles("/");
    const header = screen.getByRole("banner");
    expect(await screen.findByRole("combobox", { name: "Viewing" })).toHaveValue("self");
    expect(header).toContainElement(screen.getByRole("combobox", { name: "Viewing" }));
    expect(header).toContainElement(screen.getByRole("group", { name: "Alias view" }));
    expect(screen.queryByRole("note")).not.toBeInTheDocument();
  });

  it("mounts the keymode switcher in the header, fed by meta_keymodes", async () => {
    const { router } = await bootWithProfiles("/");
    const header = screen.getByRole("banner");
    const switcher = await screen.findByRole("group", { name: "Keymode" });
    expect(header).toContainElement(switcher);
    await userEvent.click(screen.getByRole("button", { name: "4K" }));
    await waitFor(() => {
      expect(router.state.matches[0]?.search).toEqual({ keymode: 4 });
    });
  });

  it("drops an invalid ?scope= through the root validateSearch", async () => {
    const { router } = await bootWithProfiles("/?scope=bogus&keymode=7");
    expect(router.state.matches[0]?.search).toEqual({ keymode: 7 });
    expect(await screen.findByRole("combobox", { name: "Viewing" })).toHaveValue("self");
  });

  it("shows the NotSelfBanner for scope=all", async () => {
    await bootWithProfiles("/?scope=all");
    expect(await screen.findByRole("note")).toHaveTextContent("All players: mixed, not a person");
  });

  it("keeps the global search params when following a nav link", async () => {
    const { router } = await bootWithProfiles("/?scope=all");
    await screen.findByRole("note");
    await userEvent.click(screen.getByRole("link", { name: "Settings" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/settings");
    });
    expect(router.state.matches[0]?.search).toEqual({ scope: "all" });
  });

  it("has a nav entry for Settings → Identity", async () => {
    await bootWithProfiles("/");
    expect(screen.getByRole("link", { name: "Identity" })).toHaveAttribute("href", "/settings/identity");
  });

  it("hides the scope controls during first-run setup", async () => {
    await bootApp("/setup/", { playersListProfiles: () => [SELF, ALL] });
    expect(screen.queryByRole("combobox", { name: "Viewing" })).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Keymode" })).not.toBeInTheDocument();
  });
});
