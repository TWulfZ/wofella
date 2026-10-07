import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatternDefDto } from "@/ipc/bindings";
import type { ChartWindow } from "@/features/playfield";
import { PatternGrid } from "./PatternGrid";

const TAXONOMY: PatternDefDto[] = [
  { id: "regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "exactly two notes in one column" },
  { id: "regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "three or more notes in one column" },
  { id: "regular.stream.jumpstream", axis: "7k.regular.stream", key: "js", description: "stream with two-note chords" },
];
const [MINIJACK, LONGJACK, JUMPSTREAM] = TAXONOMY as [PatternDefDto, PatternDefDto, PatternDefDto];

function example(col: number): ChartWindow {
  return {
    md5: "",
    keymode: 7,
    fromMs: 0,
    toMs: 600,
    notes: [
      { tMs: 0, col, endMs: null },
      { tMs: 300, col, endMs: null },
    ],
    timing: [{ tMs: 0, kind: "red", beatLenMs: 300, meter: 4, sv: null }],
    layout: {
      id: "k7.313_right_thumb",
      columns: ["left", "left", "left", "right", "right", "right", "right"].map((hand) => ({
        hand: hand as "left" | "right",
        finger: "index",
      })),
    },
    chartSpan: { firstMs: 0, endMs: 600 },
    audioFilename: null,
  };
}

// Jumpstream has no example on purpose: its card shows the placeholder.
const EXAMPLES: ReadonlyMap<string, ChartWindow> = new Map([
  [MINIJACK.id, example(3)],
  [LONGJACK.id, example(1)],
]);

class NoopResizeObserver {
  observe(): void {
    // Radix measures the hover card's anchor; jsdom has no layout to report.
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
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    fillStyle: "#000",
    globalAlpha: 1,
    fillRect: () => undefined,
    setTransform: () => undefined,
  } as unknown as RenderingContext);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function renderGrid(props: Partial<Parameters<typeof PatternGrid>[0]> = {}) {
  const onToggle = vi.fn<(p: PatternDefDto) => void>();
  const view = render(
    <PatternGrid
      taxonomy={TAXONOMY}
      examples={EXAMPLES}
      isActive={(p) => p.id === MINIJACK.id}
      onToggle={onToggle}
      {...props}
    />,
  );
  return { onToggle, user: userEvent.setup(), unmount: view.unmount, container: view.container };
}

function cardNames(): string[] {
  return screen
    .queryAllByRole("button")
    .filter((b) => b.hasAttribute("aria-pressed"))
    .map((b) => b.getAttribute("aria-label") ?? "");
}

// Compact pills carry no thumbnail, so a drawn canvas inside a card is the expanded density.
function thumbnailCount(): number {
  return screen
    .queryAllByRole("button")
    .filter((b) => b.hasAttribute("aria-pressed") && b.querySelector("canvas") !== null).length;
}

function alwaysCollapsed(): HTMLElement {
  return screen.getByRole("checkbox", { name: "Always collapsed" });
}

describe("PatternGrid", () => {
  it("groups the cards under one heading per axis, in taxonomy order", () => {
    renderGrid();

    expect(screen.getAllByRole("heading").map((h) => h.textContent)).toEqual(["Jack", "Stream"]);
    const jack = screen.getByRole("region", { name: "Jack" });
    expect(within(jack).getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual([
      "mj minijack",
      "lj longjack",
    ]);
  });

  it("marks active patterns as pressed and draws a preview only where an example exists", () => {
    renderGrid();

    const minijack = screen.getByRole("button", { name: "mj minijack" });
    const jumpstream = screen.getByRole("button", { name: "js jumpstream" });
    expect(minijack).toHaveAttribute("aria-pressed", "true");
    expect(jumpstream).toHaveAttribute("aria-pressed", "false");
    expect(minijack.querySelector("canvas")).not.toBeNull();
    expect(jumpstream.querySelector("canvas")).toBeNull();
  });

  it("toggles the clicked pattern", async () => {
    const { onToggle, user } = renderGrid();

    await user.click(screen.getByRole("button", { name: "js jumpstream" }));

    expect(onToggle).toHaveBeenCalledExactlyOnceWith(JUMPSTREAM);
  });

  it.each([
    ["a name, ignoring case", "JUMP", ["js jumpstream"]],
    ["a key", "lj", ["lj longjack"]],
    ["a description", "exactly two", ["mj minijack"]],
  ])("filters cards by %s", async (_, query, expected) => {
    const { user } = renderGrid();

    await user.type(screen.getByRole("searchbox", { name: "Search patterns" }), query);

    expect(cardNames()).toEqual(expected);
  });

  it("hides an axis whose patterns all fail the search", async () => {
    const { user } = renderGrid();

    await user.type(screen.getByRole("searchbox", { name: "Search patterns" }), "jack");

    expect(screen.getAllByRole("heading").map((h) => h.textContent)).toEqual(["Jack"]);
  });

  it("shows an empty state whose action clears the search", async () => {
    const { user } = renderGrid();
    const search = screen.getByRole("searchbox", { name: "Search patterns" });

    await user.type(search, "zzz");

    expect(cardNames()).toEqual([]);
    expect(screen.getByText("No pattern matches “zzz”")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(search).toHaveValue("");
    expect(search).toHaveFocus();
    expect(cardNames()).toHaveLength(3);
  });

  it("clears the search on Escape", async () => {
    const { user } = renderGrid();
    const search = screen.getByRole("searchbox", { name: "Search patterns" });

    await user.type(search, "mini");
    await user.keyboard("{Escape}");

    expect(search).toHaveValue("");
    expect(cardNames()).toHaveLength(3);
  });

  it("opens the enlarged preview with the description when a card takes keyboard focus", async () => {
    const { user } = renderGrid();

    await user.tab();
    await user.tab();
    await user.tab();
    expect(screen.getByRole("button", { name: "mj minijack" })).toHaveFocus();

    const preview = await screen.findByRole("img", { name: "Example of minijack" });
    const card = preview.closest<HTMLElement>("[data-slot=hover-card-content]");
    expect(card).not.toBeNull();
    expect(within(card ?? document.body).getByText("exactly two notes in one column")).toBeInTheDocument();
  });

  it("labels the card with its description for assistive technology", () => {
    renderGrid();

    expect(screen.getByRole("button", { name: "lj longjack" })).toHaveAccessibleDescription(
      "three or more notes in one column",
    );
  });

  it("closes the enlarged preview when focus leaves the card", async () => {
    const { user } = renderGrid();
    await user.tab();
    await user.tab();
    await user.tab();
    await screen.findByRole("img", { name: "Example of minijack" });

    await user.tab();

    await waitFor(() => {
      expect(screen.queryByRole("img", { name: "Example of minijack" })).toBeNull();
    });
  });

  it("forwards its className to the root so the panel can size it", () => {
    const { container } = renderGrid({ className: "panel-fill" });

    expect(container.firstElementChild).toHaveClass("panel-fill");
  });
});

describe("PatternGrid density", () => {
  it("starts expanded, with thumbnails, and the collapse box unticked", () => {
    renderGrid();

    expect(thumbnailCount()).toBe(2);
    expect(alwaysCollapsed()).not.toBeChecked();
  });

  it("collapses to compact pills while the search has text and expands when it is cleared", async () => {
    const { user } = renderGrid();
    const search = screen.getByRole("searchbox", { name: "Search patterns" });

    await user.type(search, "j");

    expect(cardNames()).toEqual(["mj minijack", "lj longjack", "js jumpstream"]);
    expect(thumbnailCount()).toBe(0);
    expect(screen.getByRole("button", { name: "mj minijack" })).toHaveAttribute("aria-pressed", "true");

    await user.clear(search);

    expect(thumbnailCount()).toBe(2);
  });

  it("stays compact with an empty search while Always collapsed is ticked, across remounts", async () => {
    const { user, unmount } = renderGrid();

    await user.click(alwaysCollapsed());

    expect(alwaysCollapsed()).toBeChecked();
    expect(cardNames()).toHaveLength(3);
    expect(thumbnailCount()).toBe(0);

    unmount();
    renderGrid();

    expect(alwaysCollapsed()).toBeChecked();
    expect(thumbnailCount()).toBe(0);
  });

  it("expands again when Always collapsed is unticked", async () => {
    const { user } = renderGrid();

    await user.click(alwaysCollapsed());
    await user.click(alwaysCollapsed());

    expect(thumbnailCount()).toBe(2);
  });

  it("still toggles a pattern from a compact pill", async () => {
    const { onToggle, user } = renderGrid();
    await user.click(alwaysCollapsed());

    await user.click(screen.getByRole("button", { name: "js jumpstream" }));

    expect(onToggle).toHaveBeenCalledExactlyOnceWith(JUMPSTREAM);
  });

  it("still opens the enlarged preview when a compact pill takes keyboard focus", async () => {
    const { user } = renderGrid();
    await user.click(alwaysCollapsed());

    await user.tab();
    expect(screen.getByRole("button", { name: "mj minijack" })).toHaveFocus();

    const preview = await screen.findByRole("img", { name: "Example of minijack" });
    const card = preview.closest<HTMLElement>("[data-slot=hover-card-content]");
    expect(within(card ?? document.body).getByText("exactly two notes in one column")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "lj longjack" })).toHaveAccessibleDescription(
      "three or more notes in one column",
    );
  });
});
