import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { bootApp, leafRouteId } from "@/app/testing";
import type { ProfileEntryDto, RecItemDto, RecsPreviewDto } from "@/ipc/bindings";
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

const ITEM: RecItemDto = {
  md5: "f".repeat(32),
  title: "Blue Zenith",
  artist: "xi",
  version: "7K Insane",
  creator: "Skystar",
  setId: 292301,
  beatmapId: 658127,
  rateMilli: 1000,
  needsRateCopy: false,
  isRateCopy: false,
  focusCenti: 2230,
  overallCenti: 2140,
  skillsetsCenti: [2230, 2100, 2010, 1990, 1920, 1880, 18],
  played: false,
  reasons: [
    { code: "deficit", args: ["stream", "2190"] },
    { code: "unplayed", args: [] },
  ],
};

const RECS: RecsPreviewDto = {
  scopeHash: "a".repeat(64),
  keymode: 7,
  method: "preview.band_recs@1",
  calcVersion: 527,
  state: "ready",
  anyRate: false,
  focus: "stream",
  ratingCenti: 2190,
  bandCenti: [2140, 2340],
  items: [ITEM, { ...ITEM, md5: "e".repeat(32), title: "Galaxy Collapse", rateMilli: 1500 }],
  warnings: ["uncalibrated", "k7_less_validated", "k7_tech_not_measured"],
};

function bootRecs(path = "/recommendations") {
  return bootApp(path, { playersListProfiles: () => [SELF], previewRecs: () => RECS });
}

describe("/recommendations preview", () => {
  it("renders the beta recommendations instead of the coming-soon page", async () => {
    const { calls } = await bootRecs();
    expect(await screen.findByRole("heading", { level: 1, name: "Recommended" })).toBeInTheDocument();
    expect(await screen.findByText("Beta · uncalibrated · MinaCalc 527")).toBeInTheDocument();
    const list = await screen.findByRole("list", { name: "Recommended charts" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    expect(screen.queryByText("Coming soon")).not.toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "preview_recs")?.args).toMatchObject({ keymode: 7, mode: "deficit" });
  });

  it("is reachable from the main nav", async () => {
    const { router } = await bootRecs("/");
    const nav = screen.getByRole("navigation", { name: "Main" });
    const link = within(nav).getByRole("link", { name: "Recommended" });
    expect(link).toHaveAttribute("href", "/recommendations");
    await userEvent.click(link);
    expect(await screen.findByRole("heading", { level: 1, name: "Recommended" })).toBeInTheDocument();
    expect(leafRouteId(router)).toBe("/recommendations");
  });

  it("opens the rate-copy dialog for a card that needs one", async () => {
    const { calls } = await bootApp("/recommendations", {
      playersListProfiles: () => [SELF],
      previewRecs: () => ({ ...RECS, items: [{ ...ITEM, rateMilli: 1150, needsRateCopy: true }] }),
      rateCopyPlan: (args) => ({
        previewId: "01JPREVIEW",
        md5: String(args["md5"]),
        rateMilli: Number(args["rateMilli"]),
        folder: "/mnt/e/Games/osu!/Songs/292301 xi - Blue Zenith",
        osuFilename: "xi - Blue Zenith (Skystar) [7K Insane 1.15x (230bpm)].osu",
        version: "7K Insane 1.15x (230bpm)",
        audioFilename: "audio 1.15x.ogg",
        audioExists: false,
        osuExists: false,
        refusal: null,
      }),
    });
    const generate = await screen.findByTestId("generate-rate-copy");
    expect(generate).toBeEnabled();
    await userEvent.click(generate);
    const dialog = await screen.findByRole("dialog", { name: "Generate a rate copy" });
    expect(dialog).toHaveTextContent("xi - Blue Zenith [7K Insane]");
    expect(await within(dialog).findByText("audio 1.15x.ogg")).toBeInTheDocument();
    expect(calls.filter((c) => c.cmd === "rate_copy_plan").map((c) => c.args)).toEqual([{ md5: ITEM.md5, rateMilli: 1150 }]);
  });

  it("speaks Spanish", async () => {
    await i18n.changeLanguage("es");
    await bootRecs();
    expect(await screen.findByRole("heading", { level: 1, name: "Recomendados" })).toBeInTheDocument();
    expect(await screen.findByText("Beta · sin calibrar · MinaCalc 527")).toBeInTheDocument();
  });
});
