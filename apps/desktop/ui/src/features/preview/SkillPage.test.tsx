import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProfileEntryDto, SkillPreviewDto } from "@/ipc/bindings";
import { type CommandHandlers, mockCommands } from "@/ipc/mocks";
import { i18n } from "@/shared/i18n";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { TooltipProvider } from "@/shared/ui/tooltip";
import { ALIASES, ALL_PLAYERS_4K, ready4k, ready7k, SELF_4K } from "./fixtures";
import { SkillPage } from "./SkillPage";

class NoopResizeObserver {
  observe(): void {
    // jsdom does no layout; Radix's tooltip positioning only needs the constructor.
  }
  unobserve(): void {
    // See observe.
  }
  disconnect(): void {
    // See observe.
  }
}

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", NoopResizeObserver);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const SELF_7K: ProfileEntryDto = {
  ...SELF_4K,
  scopes: [{ scopeHash: "a".repeat(64), aliasIds: [1, 2], keymode: 7 }],
};

function renderPage(
  previews: SkillPreviewDto[],
  { path = "/skill?keymode=4", profiles = [SELF_4K, ALL_PLAYERS_4K], handlers = {} }: {
    path?: string;
    profiles?: ProfileEntryDto[];
    handlers?: CommandHandlers;
  } = {},
) {
  const calls = mockCommands({
    playersListProfiles: () => profiles,
    playersListAliases: () => ALIASES,
    previewSkill: () => previews,
    ...handlers,
  });
  const view = renderWithRouter(
    <TooltipProvider>
      <SkillPage />
    </TooltipProvider>,
    { path },
  );
  return { calls, ...view };
}

describe("SkillPage ready (4K)", () => {
  it("asks for the active entry, keymode and merge override", async () => {
    const { calls } = renderPage([ready4k()], { path: "/skill?keymode=4&merge=separate" });
    await screen.findByTestId("overall");
    expect(calls.find((c) => c.cmd === "preview_skill")?.args).toEqual({
      entry: { kind: "profile", id: 1 },
      keymode: 4,
      merge: "separate",
    });
  });

  it("shows the beta badge with the calculator version and the approximate Overall", async () => {
    renderPage([ready4k({ calcVersion: 528 })]);
    expect(await screen.findByText("Beta · uncalibrated · MinaCalc 528")).toBeInTheDocument();
    expect(screen.getByTestId("overall")).toHaveTextContent("≈ 24.36");
  });

  it("shows the estimated dan with its third and explains it on hover", async () => {
    renderPage([ready4k()]);
    const dan = await screen.findByRole("button", { name: /Estimated dan/ });
    expect(dan).toHaveTextContent("≈ Gamma · mid");
    await userEvent.hover(dan);
    expect((await screen.findAllByText(/about one dan/)).length).toBeGreaterThan(0);
  });

  it("draws one radar axis per skillset the DTO sends", async () => {
    const five = ready4k().skillsets.slice(0, 5);
    renderPage([ready4k({ skillsets: five })]);
    const radar = await screen.findByRole("img", { name: /Skillset radar/ });
    expect(radar.querySelectorAll("[data-axis]")).toHaveLength(5);
    for (const name of ["Stream", "Jumpstream", "Handstream", "Stamina", "JackSpeed"]) {
      expect(within(radar).getByText(name)).toBeInTheDocument();
    }
    expect(within(radar).queryByText("Technical")).not.toBeInTheDocument();
  });

  it("lists every skillset with its value", async () => {
    renderPage([ready4k()]);
    const list = await screen.findByRole("list", { name: "Skillsets" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(7);
    expect(within(list).getByText("JackSpeed").closest("li")).toHaveTextContent("≈ 26.01");
  });

  it("explains the method in a disclosure that links to labelling", async () => {
    renderPage([ready4k()]);
    await userEvent.click(await screen.findByRole("button", { name: "How this is computed" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent(/MinaCalc/);
    expect(dialog).toHaveTextContent(/top 2/i);
    expect(dialog).toHaveTextContent(/Etterna/);
    expect(within(dialog).getByRole("link", { name: /Label/ })).toHaveAttribute("href", "/label");
  });

  it("charts the monthly trend with month labels", async () => {
    renderPage([ready4k()]);
    const trend = await screen.findByRole("img", { name: /Overall by month/ });
    expect(within(trend).getByText("Jul 2026")).toBeInTheDocument();
    expect(within(trend).getByText("Sep 2026")).toBeInTheDocument();
  });

  it("states the evidence and the localized exclusions", async () => {
    renderPage([ready4k()]);
    const evidence = await screen.findByRole("region", { name: "Plays that count" });
    expect(evidence).toHaveTextContent("312 plays count");
    expect(evidence).toHaveTextContent(/Enough plays/);
    expect(within(evidence).getByText("Not completed").closest("li")).toHaveTextContent("40");
    expect(within(evidence).getByText("Random, co-op or key-conversion mods").closest("li")).toHaveTextContent("7");
    expect(within(evidence).getByText("Not rated yet").closest("li")).toHaveTextContent("2");
  });

  it("lists the top plays with rate, goal, SSR and dominant skillset", async () => {
    renderPage([ready4k()]);
    const table = await screen.findByRole("table", { name: "Top plays" });
    const row = within(table).getByText("Blue Zenith").closest("tr");
    expect(row).not.toBeNull();
    const cells = within(row as HTMLElement);
    expect(cells.getByText("[4K Insane 1.15x]")).toBeInTheDocument();
    expect(cells.getByText("1.15x")).toBeInTheDocument();
    expect(cells.getByText("96.50%")).toBeInTheDocument();
    expect(cells.getByText("≈ 27.12")).toBeInTheDocument();
    expect(cells.getByText("JackSpeed")).toBeInTheDocument();
    expect(cells.getByText("Sep 14, 2026")).toBeInTheDocument();
  });

  it("words each warning on its own line without the 7K call to action", async () => {
    renderPage([ready4k()]);
    const warnings = await screen.findByRole("list", { name: "About these numbers" });
    expect(within(warnings).getAllByRole("listitem")).toHaveLength(2);
    expect(screen.queryByRole("link", { name: "Help calibrate: label patterns" })).not.toBeInTheDocument();
  });
});

describe("SkillPage ready (7K)", () => {
  it("shows technical as not measured, the 7K warnings and the Label call to action", async () => {
    renderPage([ready7k()], { path: "/skill?keymode=7", profiles: [SELF_7K] });
    const radar = await screen.findByRole("img", { name: /Skillset radar/ });
    expect(radar).toHaveAccessibleName(expect.stringMatching(/Technical not measured/));
    const list = screen.getByRole("list", { name: "Skillsets" });
    const technical = within(list).getByText("Technical").closest("li");
    expect(technical).toHaveTextContent("Not measured");
    expect(technical).not.toHaveTextContent("0.18");

    const warnings = screen.getByRole("list", { name: "About these numbers" });
    expect(warnings).toHaveTextContent(/less validated/);
    expect(warnings).toHaveTextContent(/long notes are not measured/i);
    expect(warnings).toHaveTextContent(/Technical is not measured/);
    expect(screen.getByRole("link", { name: "Help calibrate: label patterns" })).toHaveAttribute("href", "/label");
    expect(screen.queryByRole("button", { name: /Estimated dan/ })).not.toBeInTheDocument();
  });
});

describe("SkillPage states", () => {
  it("shows a computing state while ratings are pending", async () => {
    renderPage([ready4k({ state: "computing", overallCenti: null, skillsets: [], dan: null })]);
    expect(await screen.findByRole("status")).toHaveTextContent("Computing ratings for your plays…");
    expect(screen.queryByTestId("overall")).not.toBeInTheDocument();
  });

  it("explains how to get plays when there are none", async () => {
    renderPage([
      ready4k({
        state: "no_plays",
        overallCenti: null,
        skillsets: [],
        dan: null,
        topPlays: [],
        trend: [],
        evidence: { counted: 0, tier: "low", excluded: [] },
      }),
    ]);
    expect(await screen.findByText("No rated 4K plays yet")).toBeInTheDocument();
    expect(screen.getByText(/play .* in osu! stable/i)).toBeInTheDocument();
    expect(screen.getByText(/sync/i)).toBeInTheDocument();
  });

  it("renders one section per scope, named by its aliases, when comparing", async () => {
    const separate: ProfileEntryDto = {
      ...SELF_4K,
      mergeMode: "separate",
      scopes: [
        { scopeHash: "1".repeat(64), aliasIds: [1], keymode: 4 },
        { scopeHash: "2".repeat(64), aliasIds: [2], keymode: 4 },
      ],
    };
    renderPage([ready4k({ scopeHash: "1".repeat(64) }), ready4k({ scopeHash: "2".repeat(64), overallCenti: 1800 })], {
      profiles: [separate],
    });
    expect(await screen.findByRole("heading", { level: 2, name: "TWulfZ" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "TWulfZasdasdasd d jSS||" })).toBeInTheDocument();
    expect(screen.getAllByTestId("overall").map((e) => e.textContent)).toEqual(["≈ 24.36", "≈ 18.00"]);
  });

  it("previews the All players scope it is pointed at", async () => {
    const { calls } = renderPage([ready4k({ scopeHash: "c".repeat(64) })], { path: "/skill?keymode=4&scope=all" });
    await screen.findByTestId("overall");
    await waitFor(() => {
      expect(calls.find((c) => c.cmd === "preview_skill")?.args["entry"]).toEqual({ kind: "all_players" });
    });
  });
});

describe("SkillPage in Spanish", () => {
  it("speaks Spanish, exclusions and dan third included", async () => {
    await i18n.changeLanguage("es");
    renderPage([ready4k()]);
    expect(await screen.findByText("Beta · sin calibrar · MinaCalc 527")).toBeInTheDocument();
    expect(screen.getByTestId("overall")).toHaveTextContent("≈ 24,36");
    expect(screen.getByRole("button", { name: /Dan estimado/ })).toHaveTextContent("≈ Gamma · medio");
    const evidence = screen.getByRole("region", { name: "Jugadas que cuentan" });
    expect(within(evidence).getByText("Sin completar")).toBeInTheDocument();
    expect(within(evidence).getByText("Aún sin calcular")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cómo se calcula" })).toBeInTheDocument();
  });
});
