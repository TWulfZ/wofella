import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bootApp, leafRouteId } from "@/app/testing";
import type { LabelProgressDto, SessionPlayDto, SessionPlaysDto } from "@/ipc/bindings";
import { type CommandHandlers, emitMockEvent } from "@/ipc/mocks";

const PROGRESS: LabelProgressDto = {
  goldTotal: 42,
  goldNoPattern: 0,
  goldBlind: 0,
  perSelection: [],
  perPattern: [],
  perAxis: [],
  sessionLabels: 1,
  perDay: [{ day: "2026-10-07", gold: 1, session: 0 }],
  recent: [],
};

function play(md5Char: string, title: string, labelled = false): SessionPlayDto {
  return {
    playId: `${md5Char}0`,
    md5: md5Char.repeat(32),
    playedAt: "2026-10-07T11:00:00.000Z",
    title,
    artist: "Artist",
    version: "Hard",
    creator: "Mapper",
    stars: 4,
    keymode: 7,
    setId: null,
    label: labelled ? { eventId: "01E", pattern: null, at: "2026-10-07T11:10:00.000Z" } : null,
    goldWindows: 0,
  };
}

function session(plays: SessionPlayDto[]): SessionPlaysDto {
  return { startedAt: "2026-10-07T10:00:00.000Z", plays };
}

class NoopResizeObserver {
  observe(): void {
    // jsdom does no layout.
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

function progressHandlers(plays: () => SessionPlaysDto = () => session([play("a", "Alpha Song"), play("b", "Beta Song", true)])): CommandHandlers {
  return {
    labelTaxonomy: () => [],
    labelProgress: () => PROGRESS,
    sessionPlays: plays,
  };
}

function nav(): HTMLElement {
  return screen.getByRole("navigation", { name: "Main" });
}

function labelGroup(): HTMLElement {
  return within(nav()).getByRole("button", { name: /^Label/ });
}

describe("/label/progress", () => {
  it("renders the progress page under the Label section", async () => {
    const { router } = await bootApp("/label/progress", progressHandlers());
    expect(await screen.findByRole("heading", { level: 1, name: "Labelling progress" })).toBeInTheDocument();
    expect(leafRouteId(router)).toBe("/label/progress");
  });

  it("groups Label and Progress behind one nav entry that opens with a click", async () => {
    await bootApp("/", progressHandlers());
    const trigger = labelGroup();
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(within(nav()).queryByRole("link", { name: /Progress/ })).not.toBeInTheDocument();
    await userEvent.click(trigger);
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(within(nav()).getByRole("link", { name: "Label" })).toHaveAttribute("href", "/label");
    expect(within(nav()).getByRole("link", { name: /^Progress/ })).toHaveAttribute("href", "/label/progress");
  });

  it("opens from the keyboard, closes on Escape and gives focus back to the entry", async () => {
    await bootApp("/", progressHandlers());
    labelGroup().focus();
    await userEvent.keyboard("{Enter}");
    const label = within(nav()).getByRole("link", { name: "Label" });
    await userEvent.tab();
    expect(label).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    expect(labelGroup()).toHaveAttribute("aria-expanded", "false");
    expect(labelGroup()).toHaveFocus();
  });

  it("navigates to Progress from the group and closes it", async () => {
    const { router } = await bootApp("/", progressHandlers());
    await userEvent.click(labelGroup());
    await userEvent.click(within(nav()).getByRole("link", { name: /^Progress/ }));
    await waitFor(() => {
      expect(leafRouteId(router)).toBe("/label/progress");
    });
    expect(labelGroup()).toHaveAttribute("aria-expanded", "false");
  });

  it("marks the group and the current page as active", async () => {
    await bootApp("/label/progress", progressHandlers());
    expect(labelGroup()).toHaveAttribute("data-active", "true");
    await userEvent.click(labelGroup());
    expect(within(nav()).getByRole("link", { name: /^Progress/ })).toHaveAttribute("aria-current", "page");
    expect(within(nav()).getByRole("link", { name: "Label" })).not.toHaveAttribute("aria-current");
  });

  it("badges the pending session maps on the collapsed entry and on Progress", async () => {
    await bootApp("/", progressHandlers());
    await waitFor(() => {
      expect(labelGroup()).toHaveAccessibleName("Label, 1 map to label");
    });
    await userEvent.click(labelGroup());
    expect(within(nav()).getByRole("link", { name: "Progress, 1 map to label" })).toBeInTheDocument();
  });

  it("shows no badge when nothing is pending", async () => {
    await bootApp("/", progressHandlers(() => session([])));
    await waitFor(() => {
      expect(labelGroup()).toHaveAccessibleName("Label");
    });
    expect(within(labelGroup()).queryByTestId("pending-badge")).not.toBeInTheDocument();
  });

  it("disables the Label entry for a keymode without patterns and says why", async () => {
    await bootApp("/?keymode=4", progressHandlers());
    await waitFor(() => {
      expect(labelGroup()).toHaveAttribute("aria-disabled", "true");
    });
    expect(labelGroup()).toHaveAccessibleDescription("Pattern labelling for 4K is coming; MinaCalc ratings are available");
    await userEvent.click(labelGroup());
    expect(labelGroup()).toHaveAttribute("aria-expanded", "false");
    expect(within(nav()).queryByRole("link", { name: /^Progress/ })).not.toBeInTheDocument();
  });

  it("keeps the Label entry enabled for a keymode with patterns", async () => {
    await bootApp("/?keymode=7", progressHandlers());
    await screen.findByRole("group", { name: "Keymode" });
    expect(labelGroup()).not.toHaveAttribute("aria-disabled", "true");
    await userEvent.click(labelGroup());
    expect(labelGroup()).toHaveAttribute("aria-expanded", "true");
  });

  it("shows the empty state instead of the progress page for a keymode without patterns", async () => {
    const { calls } = await bootApp("/label/progress?keymode=4", progressHandlers());
    expect(await screen.findByRole("heading", { level: 1, name: "Pattern labelling for 4K is coming" })).toBeInTheDocument();
    expect(calls.filter((c) => c.cmd === "label_taxonomy" || c.cmd === "label_progress")).toEqual([]);
  });

  it("adds a newly played map to the list and the badge when the session event arrives", async () => {
    let plays = [play("a", "Alpha Song")];
    await bootApp("/label/progress", progressHandlers(() => session(plays)));
    const region = await screen.findByRole("region", { name: "This session" });
    expect(await within(region).findByText("Alpha Song")).toBeInTheDocument();
    await waitFor(() => {
      expect(labelGroup()).toHaveAccessibleName("Label, 1 map to label");
    });
    plays = [play("c", "Gamma Song"), ...plays];
    await emitMockEvent("sessionPlayAdded", { playId: "c0", md5: "c".repeat(32) });
    expect(await within(region).findByText("Gamma Song")).toBeInTheDocument();
    await waitFor(() => {
      expect(labelGroup()).toHaveAccessibleName("Label, 2 maps to label");
    });
  });
});
