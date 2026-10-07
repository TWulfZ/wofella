import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { bootApp, leafRouteId } from "@/app/testing";
import { i18n } from "@/shared/i18n";

describe("/skill coming soon", () => {
  it("renders its heading, the coming-soon badge and the labelled preview", async () => {
    await bootApp("/skill");
    expect(await screen.findByRole("heading", { level: 1, name: "Skill" })).toBeInTheDocument();
    expect(screen.getByText("Coming soon")).toBeInTheDocument();
    const preview = screen.getByRole("img", { name: /preview/i });
    expect(preview).toHaveAccessibleName(expect.stringMatching(/not your data/i));
    for (const axis of ["Jack", "Tech", "Speed", "Stream", "LN general", "LN tech", "LN inverse", "LN release"]) {
      expect(within(preview).getByText(axis)).toBeInTheDocument();
    }
    expect(screen.getAllByRole("listitem").length).toBeGreaterThanOrEqual(2);
  });

  it("is reachable from the main nav", async () => {
    const { router } = await bootApp("/");
    const nav = screen.getByRole("navigation", { name: "Main" });
    const link = within(nav).getByRole("link", { name: "Skill" });
    expect(link).toHaveAttribute("href", "/skill");
    await userEvent.click(link);
    expect(await screen.findByRole("heading", { level: 1, name: "Skill" })).toBeInTheDocument();
    expect(leafRouteId(router)).toBe("/skill");
  });

  it("speaks Spanish", async () => {
    await i18n.changeLanguage("es");
    await bootApp("/skill");
    expect(await screen.findByRole("heading", { level: 1, name: "Habilidad" })).toBeInTheDocument();
    expect(screen.getByText("Próximamente")).toBeInTheDocument();
  });
});
