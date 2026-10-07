import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { bootApp, leafRouteId } from "@/app/testing";
import { i18n } from "@/shared/i18n";

describe("/recommendations coming soon", () => {
  it("renders its heading, the coming-soon badge and the labelled preview", async () => {
    await bootApp("/recommendations");
    expect(await screen.findByRole("heading", { level: 1, name: "Recommendations" })).toBeInTheDocument();
    expect(screen.getByText("Coming soon")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: /preview/i })).toHaveAccessibleName(expect.stringMatching(/not real/i));
    expect(screen.getAllByRole("listitem").length).toBeGreaterThanOrEqual(2);
  });

  it("offers labelling as the thing to do meanwhile", async () => {
    await bootApp("/recommendations");
    expect(await screen.findByRole("link", { name: "Label patterns meanwhile" })).toHaveAttribute("href", "/label");
  });

  it("is reachable from the main nav", async () => {
    const { router } = await bootApp("/");
    const nav = screen.getByRole("navigation", { name: "Main" });
    const link = within(nav).getByRole("link", { name: "Recommended" });
    expect(link).toHaveAttribute("href", "/recommendations");
    await userEvent.click(link);
    expect(await screen.findByRole("heading", { level: 1, name: "Recommendations" })).toBeInTheDocument();
    expect(leafRouteId(router)).toBe("/recommendations");
  });

  it("speaks Spanish", async () => {
    await i18n.changeLanguage("es");
    await bootApp("/recommendations");
    expect(await screen.findByRole("heading", { level: 1, name: "Recomendaciones" })).toBeInTheDocument();
    expect(screen.getByText("Próximamente")).toBeInTheDocument();
  });
});
