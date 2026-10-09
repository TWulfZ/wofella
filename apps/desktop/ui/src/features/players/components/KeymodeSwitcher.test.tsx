import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { KeymodeDto } from "@/ipc/bindings";
import { mockCommands } from "@/ipc/mocks";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { keymodesQuery } from "../queries";
import { KeymodeSwitcher } from "./KeymodeSwitcher";

function keymode(n: number, hasThumb = false): KeymodeDto {
  return { keymode: n, hasPatterns: false, calculators: ["minacalc"], defaultLayout: `k${String(n)}.generic`, hasThumb };
}

function renderSwitcher(path: string, keymodes: KeymodeDto[]) {
  const calls = mockCommands({ metaKeymodes: () => keymodes });
  return { calls, ...renderWithRouter(<KeymodeSwitcher />, { path }) };
}

describe("KeymodeSwitcher", () => {
  it("is hidden when the engine indexes a single keymode", async () => {
    const { queryClient } = renderSwitcher("/", [keymode(7, true)]);
    await waitFor(() => {
      expect(queryClient.getQueryState(keymodesQuery().queryKey)?.status).toBe("success");
    });
    expect(screen.queryByRole("group", { name: "Keymode" })).not.toBeInTheDocument();
  });

  it("lists the keymodes meta_keymodes returns, in its order, with the default pressed", async () => {
    // A list the UI never saw before, so nothing in it can come from a hard-coded set (D5).
    renderSwitcher("/", [keymode(4), keymode(7, true), keymode(10)]);
    const group = await screen.findByRole("group", { name: "Keymode" });
    expect(within(group).getAllByRole("button").map((b) => b.textContent)).toEqual(["4K", "7K", "10K"]);
    expect(within(group).getByRole("button", { name: "7K" })).toHaveAttribute("aria-pressed", "true");
    expect(within(group).getByRole("button", { name: "4K" })).toHaveAttribute("aria-pressed", "false");
  });

  it("follows the URL keymode", async () => {
    renderSwitcher("/?keymode=4", [keymode(4), keymode(7, true)]);
    const group = await screen.findByRole("group", { name: "Keymode" });
    expect(within(group).getByRole("button", { name: "4K" })).toHaveAttribute("aria-pressed", "true");
  });

  it("writes the chosen keymode into the URL, keeping the other global params", async () => {
    const { router } = renderSwitcher("/?scope=all", [keymode(4), keymode(7, true)]);
    const group = await screen.findByRole("group", { name: "Keymode" });
    await userEvent.click(within(group).getByRole("button", { name: "4K" }));
    await waitFor(() => {
      expect(router.state.location.search).toMatchObject({ scope: "all", keymode: 4 });
    });
    expect(within(group).getByRole("button", { name: "4K" })).toHaveAttribute("aria-pressed", "true");
  });
});
