import { act, render, screen, waitFor, within } from "@testing-library/react";
import { createRef } from "react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatternDefDto } from "@/ipc/bindings";
import type { ChartWindow } from "@/features/playfield";
import { PATTERN_GRID_PARAMS, PatternGrid, type PatternGridHandle } from "./PatternGrid";
import { readOpenAxes, writeOpenAxes } from "./patternGridPrefs";

const TAXONOMY: PatternDefDto[] = [
  { id: "regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "exactly two notes in one column" },
  { id: "regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "three or more notes in one column" },
  { id: "regular.stream.jumpstream", axis: "7k.regular.stream", key: "js", description: "stream with two-note chords" },
  { id: "ln.release.stair_release", axis: "7k.ln.release", key: "sr", description: "releases that walk across columns" },
];
const [MINIJACK, LONGJACK, JUMPSTREAM] = TAXONOMY as [PatternDefDto, PatternDefDto, PatternDefDto, PatternDefDto];
const ALL_AXES = ["7k.regular.jack", "7k.regular.stream", "7k.ln.release"];

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

// Axis cards carry their axis id so assertions stay independent of the translated title.
function expandedAxes(): string[] {
  return screen
    .queryAllByRole("button")
    .filter((b) => b.getAttribute("aria-expanded") === "true")
    .map((b) => b.getAttribute("data-axis") ?? "");
}

function search(): HTMLElement {
  return screen.getByRole("searchbox", { name: "Search patterns" });
}

describe("PatternGrid sections", () => {
  it("splits the axes into RICE and LN master sections, in taxonomy order", () => {
    renderGrid();

    expect(screen.getAllByRole("heading", { level: 4 }).map((h) => h.textContent)).toEqual(["RICE", "LN"]);
    const rice = screen.getByRole("region", { name: "RICE" });
    const ln = screen.getByRole("region", { name: "LN" });
    const axisLabels = (root: HTMLElement): string[] =>
      within(root)
        .getAllByRole("button")
        .filter((b) => b.hasAttribute("aria-expanded"))
        .map((b) => b.getAttribute("data-axis") ?? "");
    expect(axisLabels(rice)).toEqual(["7k.regular.jack", "7k.regular.stream"]);
    expect(axisLabels(ln)).toEqual(["7k.ln.release"]);
    expect(within(ln).getByRole("button", { name: "LN release" })).toBeInTheDocument();
  });

  it("starts with every axis collapsed and no pattern card shown", () => {
    renderGrid();

    for (const name of ["Jack", "Stream", "LN release"]) {
      expect(screen.getByRole("button", { name })).toHaveAttribute("aria-expanded", "false");
    }
    expect(cardNames()).toEqual([]);
  });

  it("describes each axis card with its pattern count and how many are selected", () => {
    renderGrid();

    expect(screen.getByRole("button", { name: "Jack" })).toHaveAccessibleDescription("2 patterns 1 selected");
    expect(screen.getByRole("button", { name: "Stream" })).toHaveAccessibleDescription("1 pattern");
  });

  it("opens an axis on click into the panel it controls, and closes it again", async () => {
    const { user } = renderGrid();
    const jack = screen.getByRole("button", { name: "Jack" });

    await user.click(jack);

    expect(jack).toHaveAttribute("aria-expanded", "true");
    const panel = document.getElementById(jack.getAttribute("aria-controls") ?? "");
    expect(panel).not.toBeNull();
    expect(within(panel ?? document.body).getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual([
      "mj minijack",
      "lj longjack",
    ]);
    expect(cardNames()).toEqual(["mj minijack", "lj longjack"]);

    await user.click(jack);

    expect(jack).toHaveAttribute("aria-expanded", "false");
    expect(cardNames()).toEqual([]);
  });

  it("opens and closes an axis from the keyboard with Enter and Space", async () => {
    const { user } = renderGrid();
    const stream = screen.getByRole("button", { name: "Stream" });
    stream.focus();

    await user.keyboard("{Enter}");
    expect(stream).toHaveAttribute("aria-expanded", "true");

    await user.keyboard(" ");
    expect(stream).toHaveAttribute("aria-expanded", "false");
  });

  it("remembers which axes are open across remounts", async () => {
    const { user, unmount } = renderGrid();

    await user.click(screen.getByRole("button", { name: "Stream" }));
    unmount();
    renderGrid();

    expect(expandedAxes()).toEqual(["7k.regular.stream"]);
    expect(cardNames()).toEqual(["js jumpstream"]);
  });

  it("expands and collapses every axis at once, and persists it", async () => {
    const { user, unmount } = renderGrid();
    const expandAll = screen.getByRole("button", { name: "Expand all" });
    const collapseAll = screen.getByRole("button", { name: "Collapse all" });
    expect(collapseAll).toBeDisabled();

    await user.click(expandAll);

    expect(expandedAxes()).toEqual(ALL_AXES);
    expect(expandAll).toBeDisabled();
    unmount();
    const second = renderGrid();
    expect(expandedAxes()).toEqual(ALL_AXES);

    await second.user.click(screen.getByRole("button", { name: "Collapse all" }));

    expect(expandedAxes()).toEqual([]);
    expect(cardNames()).toEqual([]);
  });

  it("draws each axis card's note icon, tinted by its family", () => {
    renderGrid();
    const icons = ALL_AXES.map((axis) => {
      const button = document.querySelector<HTMLElement>(`button[data-axis="${axis}"]`);
      return button?.querySelector("svg[data-axis-icon]");
    });
    expect(icons.map((icon) => icon?.getAttribute("data-axis-icon"))).toEqual(ALL_AXES);
    expect(icons[0]?.getAttribute("class")).toContain("--osu-pink");
    expect(icons[2]?.getAttribute("class")).toContain("--osu-blue");
  });

  it("has no Always collapsed checkbox any more", () => {
    renderGrid();

    expect(screen.queryByRole("checkbox")).toBeNull();
  });
});

describe("PatternGrid cards", () => {
  beforeEach(() => {
    writeOpenAxes(ALL_AXES);
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

  it("opens the enlarged preview with the description when a card takes keyboard focus", async () => {
    const { user } = renderGrid();
    screen.getByRole("button", { name: "Jack" }).focus();

    await user.tab();
    expect(screen.getByRole("button", { name: "mj minijack" })).toHaveFocus();

    const preview = await screen.findByRole("img", { name: "Example of minijack" });
    const card = preview.closest<HTMLElement>("[data-slot=hover-card-content]");
    expect(card).not.toBeNull();
    expect(within(card ?? document.body).getByText("exactly two notes in one column")).toBeInTheDocument();
  });

  it("closes the enlarged preview when focus leaves the card", async () => {
    const { user } = renderGrid();
    screen.getByRole("button", { name: "Jack" }).focus();
    await user.tab();
    await screen.findByRole("img", { name: "Example of minijack" });

    await user.tab();

    await waitFor(() => {
      expect(screen.queryByRole("img", { name: "Example of minijack" })).toBeNull();
    });
  });

  it("labels the card with its description for assistive technology", () => {
    renderGrid();

    expect(screen.getByRole("button", { name: "lj longjack" })).toHaveAccessibleDescription(
      "three or more notes in one column",
    );
  });

  it("forwards its className to the root so the panel can size it", () => {
    const { container } = renderGrid({ className: "panel-fill" });

    expect(container.firstElementChild).toHaveClass("panel-fill");
  });
});

describe("PatternGrid search", () => {
  it.each([
    ["a name, ignoring case", "JUMP", ["js jumpstream"]],
    ["a key", "lj", ["lj longjack"]],
    ["a description", "exactly two", ["mj minijack"]],
  ])("opens the matching axes and shows only the cards matching %s", async (_, query, expected) => {
    const { user } = renderGrid();

    await user.type(search(), query);

    expect(cardNames()).toEqual(expected);
  });

  it("hides the axes and the master sections without a match", async () => {
    const { user } = renderGrid();

    await user.type(search(), "jack");

    expect(expandedAxes()).toEqual(["7k.regular.jack"]);
    expect(screen.queryByRole("button", { name: "Stream" })).toBeNull();
    expect(screen.queryByRole("region", { name: "LN" })).toBeNull();
    expect(screen.getAllByRole("heading", { level: 4 }).map((h) => h.textContent)).toEqual(["RICE"]);
  });

  it("restores the viewer's own open state when the search is cleared", async () => {
    const { user } = renderGrid();
    await user.click(screen.getByRole("button", { name: "Jack" }));

    await user.type(search(), "stream");
    expect(expandedAxes()).toEqual(["7k.regular.stream"]);
    await user.clear(search());

    expect(expandedAxes()).toEqual(["7k.regular.jack"]);
    expect(cardNames()).toEqual(["mj minijack", "lj longjack"]);
  });

  it("lets an axis be closed while searching without changing the saved state", async () => {
    const { user } = renderGrid();
    await user.type(search(), "two");
    expect(expandedAxes()).toEqual(["7k.regular.jack", "7k.regular.stream"]);

    await user.click(screen.getByRole("button", { name: "Stream" }));

    expect(cardNames()).toEqual(["mj minijack"]);
    await user.clear(search());
    expect(expandedAxes()).toEqual([]);
  });

  it("shows an empty state whose action clears the search", async () => {
    const { user } = renderGrid();

    await user.type(search(), "zzz");

    expect(cardNames()).toEqual([]);
    expect(screen.getByText("No pattern matches “zzz”")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(search()).toHaveValue("");
    expect(search()).toHaveFocus();
    expect(screen.getAllByRole("heading", { level: 4 })).toHaveLength(2);
  });

  it("clears the search on Escape", async () => {
    const { user } = renderGrid();

    await user.type(search(), "mini");
    await user.keyboard("{Escape}");

    expect(search()).toHaveValue("");
    expect(expandedAxes()).toEqual([]);
  });
});

describe("PatternGrid focusPattern", () => {
  function renderWithHandle() {
    const ref = createRef<PatternGridHandle>();
    const scrollIntoView = vi.fn();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { value: scrollIntoView, configurable: true, writable: true });
    const view = render(
      <PatternGrid ref={ref} taxonomy={TAXONOMY} examples={EXAMPLES} isActive={() => false} onToggle={vi.fn()} />,
    );
    const focus = (id: string): void => {
      act(() => {
        ref.current?.focusPattern(id);
      });
    };
    const reveal = (id: string): void => {
      act(() => {
        ref.current?.focusPattern(id, { moveFocus: false });
      });
    };
    return { focus, reveal, scrollIntoView, user: userEvent.setup(), unmount: view.unmount };
  }

  afterEach(() => {
    Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
  });

  it("opens the pattern's axis, scrolls its card into view and focuses it", () => {
    const { focus, scrollIntoView } = renderWithHandle();
    expect(expandedAxes()).toEqual([]);

    focus(JUMPSTREAM.id);

    expect(expandedAxes()).toEqual(["7k.regular.stream"]);
    const card = screen.getByRole("button", { name: "js jumpstream" });
    expect(card).toHaveFocus();
    expect(scrollIntoView).toHaveBeenCalledOnce();
    expect(scrollIntoView.mock.contexts[0]).toBe(card);
    expect(scrollIntoView).toHaveBeenCalledWith({ behavior: "smooth", block: "nearest" });
  });

  it("scrolls without animation when the viewer prefers reduced motion", () => {
    vi.spyOn(window, "matchMedia").mockImplementation(
      (query: string) => ({ matches: query.includes("reduce"), media: query }) as MediaQueryList,
    );
    const { focus, scrollIntoView } = renderWithHandle();

    focus(MINIJACK.id);

    expect(scrollIntoView).toHaveBeenCalledWith({ behavior: "auto", block: "nearest" });
  });

  it("highlights the card briefly", () => {
    vi.useFakeTimers();
    try {
      const { focus } = renderWithHandle();

      focus(LONGJACK.id);
      const card = screen.getByRole("button", { name: "lj longjack" });
      expect(card).toHaveAttribute("data-highlighted", "true");

      act(() => {
        vi.advanceTimersByTime(PATTERN_GRID_PARAMS.highlightMs);
      });
      expect(card).not.toHaveAttribute("data-highlighted");
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps the other open axes and opens the pattern's one for now only, leaving the stored layout alone", () => {
    writeOpenAxes(["7k.ln.release"]);
    const { focus, unmount } = renderWithHandle();

    focus(MINIJACK.id);

    expect(expandedAxes()).toEqual(["7k.regular.jack", "7k.ln.release"]);
    expect([...readOpenAxes()]).toEqual(["7k.ln.release"]);
    unmount();
    renderGrid();
    expect(expandedAxes()).toEqual(["7k.ln.release"]);
  });

  it("closes an axis it opened on the first click on its header, without storing either change", async () => {
    const { focus, user } = renderWithHandle();
    focus(MINIJACK.id);

    await user.click(screen.getByRole("button", { name: "Jack" }));

    expect(expandedAxes()).toEqual([]);
    expect([...readOpenAxes()]).toEqual([]);
  });

  it("can reveal the card without moving focus to it", () => {
    const { reveal, scrollIntoView } = renderWithHandle();
    const jack = screen.getByRole("button", { name: "Jack" });
    jack.focus();

    reveal(JUMPSTREAM.id);

    const card = screen.getByRole("button", { name: "js jumpstream" });
    expect(card).toHaveAttribute("data-highlighted", "true");
    expect(scrollIntoView.mock.contexts[0]).toBe(card);
    expect(jack).toHaveFocus();
  });

  it("clears a search that hides the pattern", async () => {
    const { focus, user } = renderWithHandle();
    await user.type(search(), "release");

    focus(MINIJACK.id);

    expect(search()).toHaveValue("");
    expect(screen.getByRole("button", { name: "mj minijack" })).toHaveFocus();
  });

  it("reopens the axis when it was closed during a matching search", async () => {
    const { focus, user } = renderWithHandle();
    await user.type(search(), "two");
    await user.click(screen.getByRole("button", { name: "Jack" }));
    expect(cardNames()).not.toContain("mj minijack");

    focus(MINIJACK.id);

    expect(search()).toHaveValue("two");
    expect(screen.getByRole("button", { name: "mj minijack" })).toHaveFocus();
  });

  it("focuses the same pattern again on a second request", () => {
    const { focus, scrollIntoView } = renderWithHandle();
    focus(MINIJACK.id);
    screen.getByRole("button", { name: "Jack" }).focus();

    focus(MINIJACK.id);

    expect(screen.getByRole("button", { name: "mj minijack" })).toHaveFocus();
    expect(scrollIntoView).toHaveBeenCalledTimes(2);
  });

  it("ignores an id outside the taxonomy", () => {
    const { focus, scrollIntoView } = renderWithHandle();

    focus("regular.jack.nope");

    expect(expandedAxes()).toEqual([]);
    expect(scrollIntoView).not.toHaveBeenCalled();
  });
});
