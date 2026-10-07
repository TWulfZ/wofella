import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { PlayfieldEffects } from "@/features/playfield";
import { PlaybackSettings } from "./PlaybackSettings";

const EFFECTS: PlayfieldEffects = { percy: true, judgements: false, combo: false, keyPress: false, lighting: false };
const ALL: Record<keyof PlayfieldEffects, boolean> = {
  percy: true,
  judgements: true,
  combo: true,
  keyPress: true,
  lighting: true,
};

function renderSettings(
  effects: PlayfieldEffects = EFFECTS,
  support: Record<keyof PlayfieldEffects, boolean> = ALL,
  onEffects = vi.fn(),
  skinLoading = false,
) {
  render(
    <PlaybackSettings
      offsetMs={0}
      onOffset={vi.fn()}
      scroll={{ kind: "osu", osuSpeed: 20, pxPerMs: 1 }}
      onScroll={vi.fn()}
      zoom={1}
      onZoom={vi.fn()}
      rate={1}
      onRate={vi.fn()}
      skin={{ options: [], folder: null, ready: true, reloading: false, onChange: vi.fn(), onReload: vi.fn() }}
      effects={effects}
      onEffects={onEffects}
      effectSupport={support}
      skinLoading={skinLoading}
      seed="42"
    />,
  );
  return onEffects;
}

describe("PlaybackSettings", () => {
  it("has no Fit window option, and the speed stays editable", () => {
    renderSettings();
    expect(screen.queryByRole("checkbox", { name: "Fit window" })).toBeNull();
    expect(screen.getByRole("spinbutton", { name: "osu! speed" })).toBeEnabled();
  });

  it("groups the skin effects, percy drawn and the rest off by default", () => {
    renderSettings();
    const group = screen.getByRole("group", { name: "Skin effects" });
    expect(group).toContainElement(screen.getByRole("checkbox", { name: "Disable percy" }));
    expect(screen.getByRole("checkbox", { name: "Disable percy" })).not.toBeChecked();
    for (const name of ["Show judgements", "Show combo", "Key press effect", "Lighting effect"]) {
      expect(screen.getByRole("checkbox", { name }), name).not.toBeChecked();
      expect(screen.getByRole("checkbox", { name }), name).toBeEnabled();
    }
  });

  it("reports each toggle as an effect patch; Disable percy turns percy off", async () => {
    const user = userEvent.setup();
    const onEffects = renderSettings();
    await user.click(screen.getByRole("checkbox", { name: "Disable percy" }));
    await user.click(screen.getByRole("checkbox", { name: "Show judgements" }));
    await user.click(screen.getByRole("checkbox", { name: "Show combo" }));
    screen.getByRole("checkbox", { name: "Key press effect" }).focus();
    await user.keyboard(" ");
    await user.click(screen.getByRole("checkbox", { name: "Lighting effect" }));
    expect(onEffects.mock.calls).toEqual([
      [{ percy: false }],
      [{ judgements: true }],
      [{ combo: true }],
      [{ keyPress: true }],
      [{ lighting: true }],
    ]);
  });

  it("disables a toggle the skin cannot draw and says why, visibly and to assistive technology", () => {
    renderSettings(EFFECTS, { ...ALL, percy: false, lighting: false });
    const percy = screen.getByRole("checkbox", { name: "Disable percy" });
    expect(percy).toBeDisabled();
    expect(percy).toHaveAccessibleDescription("Needs a skin whose long-note body is percy-style.");
    expect(screen.getByText("Needs a skin whose long-note body is percy-style.")).toBeVisible();
    expect(screen.getByRole("checkbox", { name: "Lighting effect" })).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: "Lighting effect" })).toHaveAccessibleDescription(
      "Needs a skin with lighting images.",
    );
    expect(screen.getByRole("checkbox", { name: "Show combo" })).toBeEnabled();
    expect(screen.getByRole("checkbox", { name: "Show combo" })).not.toHaveAccessibleDescription();
  });

  it("keeps judgements and combo on offer for a skin without their art, saying they show as text", async () => {
    const user = userEvent.setup();
    const onEffects = renderSettings({ ...EFFECTS, judgements: true }, { ...ALL, judgements: false, combo: false });
    const judgements = screen.getByRole("checkbox", { name: "Show judgements" });
    expect(judgements).toBeEnabled();
    expect(judgements).toBeChecked();
    expect(judgements).toHaveAccessibleDescription("This skin has no judgement images: they show as text.");
    expect(screen.getByRole("checkbox", { name: "Show combo" })).toHaveAccessibleDescription(
      "This skin has no combo font: it shows as text.",
    );
    expect(screen.getByText(/perfect autoplay/)).toBeInTheDocument();
    await user.click(judgements);
    expect(onEffects.mock.calls).toEqual([[{ judgements: false }]]);
  });

  it("shows a toggle the skin cannot draw unchecked, whatever was chosen with another skin", () => {
    renderSettings(
      { percy: false, judgements: false, combo: false, keyPress: true, lighting: true },
      { ...ALL, percy: false, keyPress: false, lighting: false },
    );
    for (const name of ["Disable percy", "Key press effect", "Lighting effect"]) {
      expect(screen.getByRole("checkbox", { name }), name).toBeDisabled();
      expect(screen.getByRole("checkbox", { name }), name).not.toBeChecked();
    }
  });

  it("says the judgements and combo follow a perfect autoplay once either is shown", () => {
    renderSettings({ ...EFFECTS, combo: true });
    expect(screen.getByText(/perfect autoplay/)).toBeInTheDocument();
  });

  it("does not mention the autoplay while every effect that follows it is off", () => {
    renderSettings();
    expect(screen.queryByText(/perfect autoplay/)).toBeNull();
  });

  it.each(["keyPress", "lighting"] as const)("says %s follows the simulated autoplay too", (id) => {
    renderSettings({ ...EFFECTS, [id]: true });
    expect(screen.getByText(/simulated perfect autoplay/)).toBeInTheDocument();
  });

  it("does not mention the autoplay for a chosen effect the skin cannot draw", () => {
    renderSettings({ ...EFFECTS, keyPress: true, lighting: true }, { ...ALL, keyPress: false, lighting: false });
    expect(screen.queryByText(/perfect autoplay/)).toBeNull();
  });

  it("while the skin loads, holds back what it cannot draw yet without blaming the skin", () => {
    renderSettings(EFFECTS, { ...ALL, percy: false, keyPress: false, lighting: false }, vi.fn(), true);
    for (const name of ["Disable percy", "Key press effect", "Lighting effect"]) {
      expect(screen.getByRole("checkbox", { name }), name).toBeDisabled();
      expect(screen.getByRole("checkbox", { name }), name).not.toHaveAccessibleDescription();
    }
    expect(screen.queryByText(/Needs a skin/)).toBeNull();
    expect(screen.getByText("Loading the skin…")).toHaveAttribute("role", "status");
    expect(screen.getByRole("checkbox", { name: "Show judgements" })).toBeEnabled();
  });
});
