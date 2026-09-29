import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { bootApp, leafRouteId, setupStatus } from "@/app/testing";

describe("first-run guard", () => {
  it("redirects to /setup/ when no install is configured", async () => {
    const { router } = await bootApp("/settings/", { setupStatus: () => setupStatus({ install: null, identityReady: false }) });
    expect(leafRouteId(router)).toBe("/setup/");
    expect(screen.queryByRole("heading", { name: "Settings" })).not.toBeInTheDocument();
  });

  it("redirects to /setup/identity when the install is set but identity is not ready", async () => {
    const { router } = await bootApp("/", { setupStatus: () => setupStatus({ identityReady: false }) });
    expect(leafRouteId(router)).toBe("/setup/identity");
  });

  it("renders the requested route when both are set", async () => {
    const { router } = await bootApp("/settings/");
    expect(leafRouteId(router)).toBe("/settings/");
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
  });

  it("does not redirect away from the setup screen itself", async () => {
    const { router } = await bootApp("/setup/", { setupStatus: () => setupStatus({ install: null, identityReady: false }) });
    expect(leafRouteId(router)).toBe("/setup/");
  });

  it("shows the localized error with Retry when setup_status fails", async () => {
    const { mockIpcError } = await import("@/ipc/mocks");
    await bootApp("/", { setupStatus: () => mockIpcError("OSU_RUNNING", {}, { retryable: true }) });
    expect(await screen.findByRole("alert")).toHaveTextContent("Close osu! and retry.");
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});

describe("app shell", () => {
  it("shows the nav and the home status with the install path", async () => {
    await bootApp("/");
    expect(screen.getByRole("navigation", { name: "Main" })).toBeInTheDocument();
    expect(await screen.findByText("/mnt/e/Games/osu!")).toBeInTheDocument();
    expect(screen.getByText("No sync has run yet.")).toBeInTheDocument();
  });
});
