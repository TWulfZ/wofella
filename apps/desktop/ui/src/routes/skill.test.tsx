import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { bootApp, leafRouteId } from "@/app/testing";
import type { ProfileEntryDto, SkillPreviewDto } from "@/ipc/bindings";
import { i18n } from "@/shared/i18n";

const SELF: ProfileEntryDto = {
  ref: { kind: "profile", id: 1 },
  profileKind: "self",
  label: "Me",
  isDefault: true,
  mergeMode: "merged",
  aliasIds: [1],
  scopes: [{ scopeHash: "a".repeat(64), aliasIds: [1], keymode: 7 }],
};

const PREVIEW: SkillPreviewDto = {
  scopeHash: "a".repeat(64),
  keymode: 7,
  method: "preview.etterna_rating@1",
  calcVersion: 527,
  state: "ready",
  overallCenti: 2082,
  skillsets: [
    { id: "stream", ratingCenti: 2190 },
    { id: "technical", ratingCenti: 18 },
  ],
  dan: null,
  evidence: { counted: 40, tier: "medium", excluded: [] },
  topPlays: [],
  trend: [],
  warnings: ["uncalibrated", "k7_less_validated", "k7_tech_not_measured"],
};

const ALL: ProfileEntryDto = {
  ref: { kind: "all_players" },
  profileKind: "all_players",
  label: "",
  isDefault: false,
  mergeMode: "merged",
  aliasIds: [1, 2],
  scopes: [{ scopeHash: "c".repeat(64), aliasIds: [1, 2], keymode: 7 }],
};

function bootSkill(path = "/skill") {
  return bootApp(path, { playersListProfiles: () => [SELF, ALL], previewSkill: () => [PREVIEW] });
}

describe("/skill preview", () => {
  it("renders the beta preview instead of the coming-soon page", async () => {
    await bootSkill();
    expect(await screen.findByRole("heading", { level: 1, name: "Skill" })).toBeInTheDocument();
    expect(await screen.findByText("Beta · uncalibrated · MinaCalc 527")).toBeInTheDocument();
    expect(screen.getByTestId("overall")).toHaveTextContent("≈ 20.82");
    expect(screen.queryByText("Coming soon")).not.toBeInTheDocument();
  });

  it("keeps the not-your-data banner for All players", async () => {
    await bootSkill("/skill?scope=all");
    expect(await screen.findByRole("note")).toHaveTextContent("All players: mixed, not a person");
    expect(await screen.findByTestId("overall")).toBeInTheDocument();
  });

  it("is reachable from the main nav", async () => {
    const { router } = await bootSkill("/");
    const nav = screen.getByRole("navigation", { name: "Main" });
    const link = within(nav).getByRole("link", { name: "Skill" });
    expect(link).toHaveAttribute("href", "/skill");
    await userEvent.click(link);
    expect(await screen.findByRole("heading", { level: 1, name: "Skill" })).toBeInTheDocument();
    expect(leafRouteId(router)).toBe("/skill");
  });

  it("speaks Spanish", async () => {
    await i18n.changeLanguage("es");
    await bootSkill();
    expect(await screen.findByRole("heading", { level: 1, name: "Habilidad" })).toBeInTheDocument();
    expect(await screen.findByText("Beta · sin calibrar · MinaCalc 527")).toBeInTheDocument();
  });
});
