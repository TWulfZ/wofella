import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { resetJobTray } from "@/features/jobs";
import type { AliasListDto, DecideAliasInput } from "@/ipc/bindings";
import { mockCommands, mockIpcError } from "@/ipc/mocks";
import { navigatedPath, renderWithRouter } from "@/shared/testing/renderWithRouter";
import { noMatchAliasList, pilotAliasList } from "../fixtures";
import { IdentityWizard } from "./IdentityWizard";

// The tray store is global; a running job hydrated by one test would disable Confirm in the next.
beforeEach(() => {
  resetJobTray();
});

function renderWizard(list: AliasListDto, extra: Parameters<typeof mockCommands>[0] = {}) {
  const calls = mockCommands({
    playersListAliases: () => list,
    playersDecideAlias: () => ({ ...list, wizardNeeded: false }),
    jobsList: () => [],
    ...extra,
  });
  const view = renderWithRouter(<IdentityWizard mode="wizard" />, { path: "/setup/identity" });
  return { calls, ...view };
}

async function rows(): Promise<HTMLElement[]> {
  const table = await screen.findByRole("table");
  return within(table).getAllByRole("row").slice(1);
}

function at(list: HTMLElement[], index: number): HTMLElement {
  const element = list[index];
  if (element === undefined) {
    throw new Error(`no row ${index}`);
  }
  return element;
}

function rowCheckbox(row: HTMLElement): HTMLElement {
  return within(row).getByRole("checkbox");
}

describe("IdentityWizard", () => {
  it("lists auto rows first, ticked, with the chip; every other row unticked in the DTO order", async () => {
    renderWizard(pilotAliasList());
    const body = await rows();

    expect(body.map((r) => within(r).getByTestId("alias-name").textContent)).toEqual([
      "TWulfZ",
      "TWulfZasdasdasd d jSS||",
      "(no name)\"\"",
      "W",
      "Rosalind",
      "s",
      "w",
      "Kovacs",
      "Wulf",
      "Sterling",
    ]);
    for (const r of body.slice(0, 2)) {
      expect(rowCheckbox(r)).toBeChecked();
      expect(within(r).getByText("matches your osu! login")).toBeInTheDocument();
    }
    for (const r of body.slice(2)) {
      expect(rowCheckbox(r)).not.toBeChecked();
      expect(within(r).queryByText("matches your osu! login")).not.toBeInTheDocument();
    }
    expect(screen.queryByText(/Tick the names that are yours/)).not.toBeInTheDocument();
  });

  it("shows play counts, keymodes with 7K first, and the online/offline split", async () => {
    renderWizard(pilotAliasList());
    const [first] = await rows();
    expect(first).toBeDefined();
    const text = first?.textContent ?? "";
    expect(text).toContain("3,019");
    expect(text.indexOf("7K 3,000")).toBeLessThan(text.indexOf("4K 19"));
    expect(text).toContain("270 online · 2,749 offline");
  });

  it("renders the empty name as (no name) with the raw quotes in monospace", async () => {
    renderWizard(pilotAliasList());
    const body = await rows();
    const empty = at(body, 2);
    expect(within(empty).getByText("(no name)")).toBeInTheDocument();
    expect(within(empty).getByText('""').tagName).toBe("CODE");
  });

  it("with no auto match nothing is ticked and the prompt shows", async () => {
    renderWizard(noMatchAliasList());
    const body = await rows();
    for (const r of body) {
      expect(rowCheckbox(r)).not.toBeChecked();
    }
    expect(screen.getByText(/Tick the names that are yours/)).toBeInTheDocument();
  });

  it("Select all is tri-state and ticks every row", async () => {
    renderWizard(pilotAliasList());
    await rows();
    const selectAll = screen.getByRole("checkbox", { name: "Select all" });
    expect(selectAll).toHaveAttribute("data-state", "indeterminate");

    await userEvent.click(selectAll);
    for (const r of await rows()) {
      expect(rowCheckbox(r)).toBeChecked();
    }
    expect(selectAll).toBeChecked();

    await userEvent.click(selectAll);
    for (const r of await rows()) {
      expect(rowCheckbox(r)).not.toBeChecked();
    }
  });

  it("Confirm sends ticked → me, unticked auto → not_me, others omitted, completesWizard, then leaves", async () => {
    const { calls } = renderWizard(pilotAliasList());
    const body = await rows();
    // Untick the cfg-string alias (auto) and tick "" and W.
    await userEvent.click(rowCheckbox(at(body, 1)));
    await userEvent.click(rowCheckbox(at(body, 2)));
    await userEvent.click(rowCheckbox(at(body, 3)));

    await userEvent.click(screen.getByRole("button", { name: "Confirm" }));

    await waitFor(() => {
      expect(calls.some((c) => c.cmd === "players_decide_alias")).toBe(true);
    });
    const input = calls.find((c) => c.cmd === "players_decide_alias")?.args["input"] as DecideAliasInput;
    expect(input.completesWizard).toBe(true);
    expect([...input.decisions].sort((a, b) => a.aliasId - b.aliasId)).toEqual([
      { aliasId: 1, decision: "me" },
      { aliasId: 2, decision: "not_me" },
      { aliasId: 3, decision: "me" },
      { aliasId: 4, decision: "me" },
    ]);
    expect(await navigatedPath()).toBe("/");
  });

  it("shows the empty state when there are no aliases and no sync is running", async () => {
    renderWizard(pilotAliasList({ aliases: [], wizardNeeded: false }));
    expect(await screen.findByText(/No plays yet/)).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("shows the localized error with the slice key when the list fails", async () => {
    renderWizard(pilotAliasList(), {
      playersListAliases: () =>
        mockIpcError("NOT_FOUND", { aliasId: "9" }, { messageKey: "players.error.unknown_alias" }),
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("Name #9 no longer exists.");
  });

  it("disables Confirm while a sync or identity refresh is running", async () => {
    renderWizard(pilotAliasList(), {
      jobsList: () => [
        { id: "01JREFRESH", kind: "refresh_identity", status: "running", started: null, ended: null, summary: null, error: null },
      ],
    });
    await rows();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Confirm" })).toBeDisabled();
    });
    expect(screen.getByText(/Importing your plays/)).toBeInTheDocument();
  });
});

describe("IdentityWizard in settings mode", () => {
  it("shows decision chips and saves without completing the wizard", async () => {
    const list = pilotAliasList({ wizardNeeded: false });
    const decided = list.aliases.map((a) =>
      a.rawName === "W" ? { ...a, decision: "me" as const, selected: true, inSelfProfile: true } : a.rawName === "Rosalind" ? { ...a, decision: "not_me" as const } : a,
    );
    const calls = mockCommands({
      playersListAliases: () => ({ ...list, aliases: decided }),
      playersDecideAlias: () => ({ ...list, aliases: decided }),
      jobsList: () => [],
    });
    renderWithRouter(<IdentityWizard mode="settings" />, { path: "/settings/identity" });

    const body = await rows();
    expect(within(at(body, 3)).getByText("confirmed")).toBeInTheDocument();
    expect(within(at(body, 4)).getByText("not you")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(calls.some((c) => c.cmd === "players_decide_alias")).toBe(true);
    });
    const input = calls.find((c) => c.cmd === "players_decide_alias")?.args["input"] as DecideAliasInput;
    expect(input.completesWizard).toBe(false);
    expect(screen.queryByTestId("location")).not.toBeInTheDocument();
  });
});
