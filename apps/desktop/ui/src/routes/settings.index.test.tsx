import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { bootApp } from "@/app/testing";
import { i18n, LANGUAGE_STORAGE_KEY } from "@/shared/i18n";

describe("/settings/", () => {
  it("shows the data dir, logs dir and version", async () => {
    await bootApp("/settings/");
    expect(await screen.findByText("/home/pilot/.local/share/wolluf")).toBeInTheDocument();
    expect(screen.getByText("/home/pilot/.local/share/wolluf/logs")).toBeInTheDocument();
    expect(screen.getByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Choose which names are yours" })).toHaveAttribute("href", "/settings/identity");
  });

  it("switches language and persists the choice", async () => {
    await bootApp("/settings/");
    await userEvent.click(await screen.findByRole("radio", { name: "Español" }));
    await waitFor(() => {
      expect(i18n.language).toBe("es");
    });
    expect(window.localStorage.getItem(LANGUAGE_STORAGE_KEY)).toBe("es");
    expect(await screen.findByRole("heading", { name: "Ajustes" })).toBeInTheDocument();
  });

  it("opens the logs folder through app_open_logs_dir", async () => {
    const { calls } = await bootApp("/settings/", { appOpenLogsDir: () => null });
    await userEvent.click(await screen.findByRole("button", { name: "Open logs folder" }));
    await waitFor(() => {
      expect(calls.map((c) => c.cmd)).toContain("app_open_logs_dir");
    });
  });
});
