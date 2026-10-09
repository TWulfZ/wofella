import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProfileEntryDto, RecItemDto, RecsPreviewDto } from "@/ipc/bindings";
import { type CommandHandlers, mockCommands, mockIpcError } from "@/ipc/mocks";
import { i18n } from "@/shared/i18n";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { TooltipProvider } from "@/shared/ui/tooltip";
import { ALL_PLAYERS_4K, recItem, recs4k, recs7k, SELF_4K } from "./fixtures";
import { RecommendationsPage } from "./RecommendationsPage";

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
  recs: RecsPreviewDto | ((args: Record<string, unknown>) => RecsPreviewDto),
  {
    path = "/recommendations?keymode=4",
    profiles = [SELF_4K, ALL_PLAYERS_4K],
    handlers = {},
    onGenerate,
  }: {
    path?: string;
    profiles?: ProfileEntryDto[];
    handlers?: CommandHandlers;
    onGenerate?: (item: RecItemDto) => void;
  } = {},
) {
  const user = userEvent.setup();
  const calls = mockCommands({
    playersListProfiles: () => profiles,
    previewRecs: typeof recs === "function" ? recs : () => recs,
    ...handlers,
  });
  const view = renderWithRouter(
    <TooltipProvider>
      <RecommendationsPage onGenerateRateCopy={onGenerate} />
    </TooltipProvider>,
    { path },
  );
  const recsCalls = () => calls.filter((c) => c.cmd === "preview_recs").map((c) => c.args);
  return { user, calls, recsCalls, ...view };
}

async function cards(listName = "Recommended charts"): Promise<HTMLElement[]> {
  const list = await screen.findByRole("list", { name: listName });
  return within(list).getAllByRole("listitem");
}

async function card(title: string, listName?: string): Promise<HTMLElement> {
  await cards(listName);
  const heading = screen.getByRole("heading", { name: title });
  const item = heading.closest("li");
  expect(item).not.toBeNull();
  return item as HTMLElement;
}

describe("RecommendationsPage header and modes", () => {
  it("shows the title, the beta badge and a method disclosure about the band", async () => {
    const { user } = renderPage(recs4k({ calcVersion: 528 }));
    expect(await screen.findByText("Beta · uncalibrated · MinaCalc 528")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "How this is computed" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent(/band around your rating/i);
    expect(dialog).toHaveTextContent(/leads the chart/i);
    expect(dialog).toHaveTextContent(/one map per beatmap set/i);
    expect(dialog).toHaveTextContent(/not played first/i);
    expect(dialog).toHaveTextContent(/uncalibrated/i);
    expect(within(dialog).getByRole("link", { name: /Label/ })).toHaveAttribute("href", "/label");
  });

  it("starts on Deficit for the active entry, keymode and merge mode", async () => {
    const { recsCalls } = renderPage(recs4k(), { path: "/recommendations?keymode=4&merge=separate" });
    await cards();
    expect(screen.getByRole("tab", { name: /Deficit/ })).toHaveAttribute("aria-selected", "true");
    expect(recsCalls()[0]).toEqual({
      entry: { kind: "profile", id: 1 },
      keymode: 4,
      mode: "deficit",
      skillset: null,
      merge: "separate",
    });
  });

  it("switches to Push and to Skillset, which starts on the current focus and follows the picker", async () => {
    const { user, recsCalls } = renderPage((args) =>
      recs4k({ focus: args["mode"] === "push" ? "overall" : typeof args["skillset"] === "string" ? args["skillset"] : "jumpstream" }),
    );
    await cards();
    await user.click(screen.getByRole("tab", { name: /Push/ }));
    await waitFor(() => {
      expect(recsCalls().at(-1)).toMatchObject({ mode: "push", skillset: null });
    });
    expect(screen.getByRole("tab", { name: /Push/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByRole("radiogroup", { name: "Skillset" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: /Deficit/ }));
    await waitFor(() => {
      expect(screen.getByTestId("recs-summary")).toHaveTextContent("Jumpstream");
    });
    await user.click(screen.getByRole("tab", { name: /^Skillset/ }));
    await waitFor(() => {
      expect(recsCalls().at(-1)).toMatchObject({ mode: "skillset", skillset: "jumpstream" });
    });
    const picker = screen.getByRole("radiogroup", { name: "Skillset" });
    expect(within(picker).getByRole("radio", { name: /Jumpstream/ })).toBeChecked();
    await user.click(within(picker).getByRole("radio", { name: /Chordjack/ }));
    await waitFor(() => {
      expect(recsCalls().at(-1)).toMatchObject({ mode: "skillset", skillset: "chordjack" });
    });
  });

  it("starts Skillset on the first measured skillset when the focus is Overall", async () => {
    const { user, recsCalls } = renderPage(recs4k({ focus: "overall" }));
    await cards();
    await user.click(screen.getByRole("tab", { name: /^Skillset/ }));
    await waitFor(() => {
      expect(recsCalls().at(-1)).toMatchObject({ mode: "skillset", skillset: "stream" });
    });
  });

  it("disables 7K Technical and Stamina in the picker and says they are not measured", async () => {
    const { user } = renderPage(recs7k(), { path: "/recommendations?keymode=7", profiles: [SELF_7K] });
    await cards();
    await user.click(screen.getByRole("tab", { name: /^Skillset/ }));
    const picker = await screen.findByRole("radiogroup", { name: "Skillset" });
    expect(within(picker).getByRole("radio", { name: /Technical/ })).toBeDisabled();
    expect(within(picker).getByRole("radio", { name: /Stamina/ })).toBeDisabled();
    expect(within(picker).getByRole("radio", { name: /Stream$/ })).toBeEnabled();
    await user.hover(within(picker).getByText("Technical"));
    expect((await screen.findAllByText("Not measured on 7K")).length).toBeGreaterThan(0);
  });

  it("states the focus, the rating and the band", async () => {
    renderPage(recs4k());
    const summary = await screen.findByTestId("recs-summary");
    expect(summary).toHaveTextContent("Focus: Jumpstream");
    expect(summary).toHaveTextContent("Your rating ≈ 21.05");
    expect(summary).toHaveTextContent("Band ≈ 20.55–22.55");
  });
});

describe("RecommendationsPage items", () => {
  it("lists one card per item with chart, artist, version and creator", async () => {
    renderPage(recs4k());
    expect(await cards()).toHaveLength(4);
    const blue = await card("Blue Zenith");
    expect(blue).toHaveTextContent("xi");
    expect(blue).toHaveTextContent("[4K Insane]");
    expect(blue).toHaveTextContent("Skystar");
  });

  it("badges each rate the osu! way", async () => {
    renderPage(recs4k());
    expect(within(await card("Blue Zenith")).getByTestId("rate")).toHaveTextContent("1.50x DT");
    expect(within(await card("Galaxy Collapse")).getByTestId("rate")).toHaveTextContent("0.75x HT");
    expect(within(await card("Freedom Dive")).getByTestId("rate")).toHaveTextContent(/^1\.15x$/);
    const kamui = await card("Kamui");
    expect(within(kamui).getByTestId("rate")).toHaveTextContent(/^1\.00x$/);
    expect(within(kamui).getByText("Rate copy")).toBeInTheDocument();
    expect(within(await card("Blue Zenith")).queryByText("Rate copy")).not.toBeInTheDocument();
  });

  it("shows the focus MSD, Overall and whether it was played", async () => {
    renderPage(recs4k());
    const blue = await card("Blue Zenith");
    expect(blue).toHaveTextContent("Jumpstream ≈ 21.80");
    expect(blue).toHaveTextContent("Overall ≈ 21.40");
    expect(within(blue).getByText("New")).toBeInTheDocument();
    expect(within(await card("Galaxy Collapse")).getByText("Played")).toBeInTheDocument();
  });

  it("shows Overall once when it is the focus", async () => {
    renderPage(recs4k({ focus: "overall", items: [recItem({ reasons: [{ code: "push", args: ["2155"] }] })] }));
    const blue = await card("Blue Zenith");
    expect(within(blue).getAllByText(/^Overall/)).toHaveLength(1);
    expect(blue).toHaveTextContent("Aims a little above your Overall, around ≈ 21.55");
  });

  it("words every reason with its arguments", async () => {
    renderPage(recs4k());
    const blue = await card("Blue Zenith");
    expect(within(blue).getByText("Trains Jumpstream, your weakest skillset (≈ 21.05)")).toBeInTheDocument();
    expect(within(blue).getByText("You have not played it yet")).toBeInTheDocument();
    expect(within(await card("Galaxy Collapse")).getByText("You have played it before")).toBeInTheDocument();
    expect(within(await card("Freedom Dive")).getByText(/Needs a 1\.15x rate copy/)).toBeInTheDocument();
    expect(within(await card("Kamui")).getByText("Uses a rate copy already in your library")).toBeInTheDocument();
  });

  it("words the Skillset reason and keeps an unknown code readable", async () => {
    renderPage(
      recs4k({
        items: [
          recItem({
            reasons: [
              { code: "skillset", args: ["chordjack"] },
              { code: "brand_new", args: [] },
            ],
          }),
        ],
      }),
    );
    const blue = await card("Blue Zenith");
    expect(within(blue).getByText("Chordjack leads this chart")).toBeInTheDocument();
    expect(within(blue).getByText("brand_new")).toBeInTheDocument();
  });

  it("copies the name in the form osu!'s search finds", async () => {
    const { user } = renderPage(recs4k());
    const blue = await card("Blue Zenith");
    await user.click(within(blue).getByRole("button", { name: "Copy name" }));
    await expect(navigator.clipboard.readText()).resolves.toBe("xi - Blue Zenith [4K Insane]");
    expect(within(blue).getByRole("status")).toHaveTextContent("Copied");
  });

  it("offers a disabled rate copy action only where one is needed until it is wired", async () => {
    const { user } = renderPage(recs4k());
    const dive = await card("Freedom Dive");
    const generate = within(dive).getByTestId("generate-rate-copy");
    expect(generate).toBeDisabled();
    expect(generate).toHaveTextContent("Generate rate copy");
    expect(screen.getAllByTestId("generate-rate-copy")).toHaveLength(1);
    await user.hover(generate.closest("span") ?? generate);
    expect((await screen.findAllByText("Coming in this beta")).length).toBeGreaterThan(0);
  });

  it("hands the item to onGenerateRateCopy when it is wired", async () => {
    const onGenerate = vi.fn();
    const { user } = renderPage(recs4k(), { onGenerate });
    const generate = within(await card("Freedom Dive")).getByTestId("generate-rate-copy");
    expect(generate).toBeEnabled();
    await user.click(generate);
    expect(onGenerate).toHaveBeenCalledWith(expect.objectContaining({ md5: "2".repeat(32), rateMilli: 1150 }));
  });
});

describe("RecommendationsPage states", () => {
  it("shows the computing placeholder when nothing is cached", async () => {
    renderPage(recs4k({ state: "computing", items: [] }));
    expect(await screen.findByRole("status")).toHaveTextContent("Computing ratings for your plays…");
    expect(screen.queryByRole("list", { name: "Recommended charts" })).not.toBeInTheDocument();
  });

  it("keeps the cached picks on screen while ratings update", async () => {
    renderPage(recs4k({ state: "computing" }));
    expect(await cards()).toHaveLength(4);
    expect(screen.getByText("Updating ratings…")).toBeInTheDocument();
    expect(screen.queryByText("Computing ratings for your plays…")).not.toBeInTheDocument();
  });

  it("explains how to get a rating when there is none", async () => {
    renderPage(recs4k({ state: "no_rating", focus: "", ratingCenti: 0, bandCenti: [0, 0], items: [] }));
    expect(await screen.findByText("No 4K rating yet")).toBeInTheDocument();
    expect(screen.getByText(/complete some 4K charts/i)).toHaveTextContent(/sync/i);
    expect(screen.queryByTestId("recs-summary")).not.toBeInTheDocument();
  });

  it("explains the band and suggests enabling any rate when nothing fits", async () => {
    renderPage(recs4k({ anyRate: false, items: [] }));
    expect(await screen.findByText("No chart fits your band yet")).toBeInTheDocument();
    expect(screen.getByText(/Jumpstream between ≈ 20.55 and ≈ 22.55/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Enable any rate in Settings" })).toHaveAttribute("href", "/settings");
  });

  it("does not suggest any rate when it is already on", async () => {
    renderPage(recs4k({ anyRate: true, items: [] }));
    expect(await screen.findByText("No chart fits your band yet")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Enable any rate in Settings" })).not.toBeInTheDocument();
  });

  it("shows the 7K warnings with the Label call to action", async () => {
    renderPage(recs7k(), { path: "/recommendations?keymode=7", profiles: [SELF_7K] });
    const warnings = await screen.findByRole("list", { name: "About these numbers" });
    expect(warnings).toHaveTextContent(/less validated/);
    expect(screen.getByRole("link", { name: "Help calibrate: label patterns" })).toHaveAttribute("href", "/label");
  });

  it("words a failure and retries", async () => {
    let fail = true;
    const { user } = renderPage(() => {
      if (fail) {
        mockIpcError("INTERNAL");
      }
      return recs4k();
    });
    const alert = await screen.findByRole("alert");
    fail = false;
    await user.click(within(alert).getByRole("button", { name: "Retry" }));
    expect(await cards()).toHaveLength(4);
  });
});

describe("RecommendationsPage in Spanish", () => {
  it("speaks Spanish, rates and reasons included", async () => {
    await i18n.changeLanguage("es");
    renderPage(recs4k());
    expect(await screen.findByText("Beta · sin calibrar · MinaCalc 527")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Déficit/ })).toBeInTheDocument();
    expect(screen.getByTestId("recs-summary")).toHaveTextContent("Banda ≈ 20,55–22,55");
    const blue = await card("Blue Zenith", "Mapas recomendados");
    expect(within(blue).getByTestId("rate")).toHaveTextContent("1,50x DT");
    expect(within(blue).getByText("Entrena Jumpstream, tu skillset más débil (≈ 21,05)")).toBeInTheDocument();
    expect(within(blue).getByRole("button", { name: "Copiar nombre" })).toBeInTheDocument();
    expect(within(await card("Freedom Dive", "Mapas recomendados")).getByTestId("generate-rate-copy")).toHaveTextContent("Generar copia con rate");
  });
});
