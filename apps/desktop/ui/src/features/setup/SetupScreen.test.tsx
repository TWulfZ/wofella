import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { InstallCandidateDto, InstallDto } from "@/ipc/bindings";
import { mockCommands, mockIpcError } from "@/ipc/mocks";
import { navigatedPath, renderWithRouter } from "@/shared/testing/renderWithRouter";
import { SetupScreen } from "./SetupScreen";

const REGISTRY: InstallCandidateDto = {
  path: "/mnt/e/Games/osu!",
  source: "registry",
  valid: true,
  osuDbVersion: 20260924,
  missing: [],
};
const DRIVE_SCAN: InstallCandidateDto = {
  path: "/mnt/d/osu!",
  source: "drive_scan",
  valid: false,
  osuDbVersion: null,
  missing: ["osu!.db", "scores.db"],
};
const INSTALL: InstallDto = { id: 7, rootPath: REGISTRY.path, osuDbVersion: 20260924, detectedAt: "2026-09-28T10:00:00.000Z" };

function renderSetup() {
  return renderWithRouter(<SetupScreen />, { path: "/setup" });
}

describe("SetupScreen", () => {
  it("lists candidates with their source badge, osu!.db version and missing files", async () => {
    mockCommands({ setupDetectInstalls: () => [REGISTRY, DRIVE_SCAN] });
    renderSetup();

    const registry = await screen.findByRole("radio", { name: /\/mnt\/e\/Games\/osu!/ });
    const registryItem = registry.closest("li");
    expect(registryItem).not.toBeNull();
    expect(within(registryItem as HTMLElement).getByText("Registry")).toBeInTheDocument();
    expect(within(registryItem as HTMLElement).getByText("osu!.db 20260924")).toBeInTheDocument();
    expect(registry).toBeChecked();

    const scanItem = screen.getByRole("radio", { name: /\/mnt\/d\/osu!/ }).closest("li") as HTMLElement;
    expect(within(scanItem).getByText("Drive scan")).toBeInTheDocument();
    expect(within(scanItem).getByText("Missing: osu!.db, scores.db")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /\/mnt\/d\/osu!/ })).toBeDisabled();
  });

  it("shows the two first-run steps with the osu! folder step current", async () => {
    mockCommands({ setupDetectInstalls: () => [REGISTRY] });
    renderSetup();

    const steps = await screen.findByRole("list", { name: "Setup steps" });
    const items = within(steps).getAllByRole("listitem");
    expect(items.map((li) => li.textContent)).toEqual(["1osu! folder", "2Identity"]);
    expect(items[0]).toHaveAttribute("aria-current", "step");
    expect(items[1]).not.toHaveAttribute("aria-current");
  });

  it("marks a valid candidate as ready in text, not only by colour", async () => {
    mockCommands({ setupDetectInstalls: () => [REGISTRY, DRIVE_SCAN] });
    renderSetup();

    const registryItem = (await screen.findByRole("radio", { name: /\/mnt\/e\/Games\/osu!/ })).closest("li") as HTMLElement;
    expect(within(registryItem).getByText("Ready to use")).toBeInTheDocument();
    const scanItem = screen.getByRole("radio", { name: /\/mnt\/d\/osu!/ }).closest("li") as HTMLElement;
    expect(within(scanItem).queryByText("Ready to use")).not.toBeInTheDocument();
    expect(within(scanItem).getByText("Not a valid install")).toBeInTheDocument();
  });

  it("confirming a valid install sets the path, then starts sync_plays, then goes to /setup/identity", async () => {
    const calls = mockCommands({
      setupDetectInstalls: () => [REGISTRY],
      setupSetInstallPath: () => INSTALL,
      jobsStart: () => "01JOB",
    });
    renderSetup();

    await userEvent.click(await screen.findByRole("button", { name: "Use this install" }));

    expect(await navigatedPath()).toBe("/setup/identity");
    const commands = calls.filter((c) => !c.cmd.startsWith("plugin:"));
    expect(commands.map((c) => [c.cmd, c.args])).toEqual([
      ["setup_detect_installs", {}],
      ["setup_set_install_path", { path: REGISTRY.path }],
      ["jobs_start", { request: { kind: "sync_plays", installId: 7 } }],
    ]);
  });

  it("an invalid folder shows the OSU_DIR_NOT_FOUND text with the path and stays on the screen", async () => {
    mockCommands(
      {
        setupDetectInstalls: () => [],
        setupSetInstallPath: ({ path }) => mockIpcError("OSU_DIR_NOT_FOUND", { path: String(path) }),
      },
      { "plugin:dialog|open": () => "/home/pilot/Downloads" },
    );
    renderSetup();

    await userEvent.click(await screen.findByRole("button", { name: "Browse…" }));
    await userEvent.click(screen.getByRole("button", { name: "Use this install" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("No valid osu! stable install at “/home/pilot/Downloads”.");
    expect(screen.queryByTestId("location")).not.toBeInTheDocument();
  });

  it("a lazer folder shows setup.error.lazer_not_supported", async () => {
    mockCommands(
      {
        setupDetectInstalls: () => [],
        setupSetInstallPath: ({ path }) =>
          mockIpcError("UNSUPPORTED_FORMAT", { path: String(path) }, { messageKey: "setup.error.lazer_not_supported" }),
      },
      { "plugin:dialog|open": () => "/home/pilot/.local/share/osu" },
    );
    renderSetup();

    await userEvent.click(await screen.findByRole("button", { name: "Browse…" }));
    await userEvent.click(screen.getByRole("button", { name: "Use this install" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "“/home/pilot/.local/share/osu” is an osu!lazer install. wofella reads osu! stable only.",
    );
  });

  it("Browse uses the dialog result as the selected folder", async () => {
    const calls = mockCommands(
      { setupDetectInstalls: () => [REGISTRY], setupSetInstallPath: () => INSTALL, jobsStart: () => "01JOB" },
      { "plugin:dialog|open": () => "/mnt/f/osu-portable" },
    );
    renderSetup();

    await userEvent.click(await screen.findByRole("button", { name: "Browse…" }));
    expect(screen.getByRole("radio", { name: /\/mnt\/f\/osu-portable/ })).toBeChecked();
    await userEvent.click(screen.getByRole("button", { name: "Use this install" }));

    await waitFor(() => {
      expect(calls.find((c) => c.cmd === "setup_set_install_path")?.args).toEqual({ path: "/mnt/f/osu-portable" });
    });
    const dialog = calls.find((c) => c.cmd === "plugin:dialog|open");
    expect(dialog?.args).toMatchObject({ options: { directory: true, multiple: false } });
  });

  it("a cancelled Browse keeps the current selection", async () => {
    mockCommands({ setupDetectInstalls: () => [REGISTRY] }, { "plugin:dialog|open": () => null });
    renderSetup();

    await userEvent.click(await screen.findByRole("button", { name: "Browse…" }));

    expect(screen.getByRole("radio", { name: /\/mnt\/e\/Games\/osu!/ })).toBeChecked();
  });

  it("with no candidates, Confirm stays disabled until a folder is browsed", async () => {
    mockCommands({ setupDetectInstalls: () => [] });
    renderSetup();

    expect(await screen.findByText(/No osu! stable install was found automatically/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use this install" })).toBeDisabled();
  });
});
