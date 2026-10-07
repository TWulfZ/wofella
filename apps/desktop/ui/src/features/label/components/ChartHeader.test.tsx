import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/ipc/mocks";
import type { LabelWindow } from "../types";
import {
  backgroundDataUrl,
  CHART_HEADER_PARAMS,
  type ChartDetails,
  ChartHeader,
  type ChartHeaderProps,
} from "./ChartHeader";

afterEach(() => {
  vi.unstubAllGlobals();
});

const WINDOW: LabelWindow = {
  anchor: { md5: "a".repeat(32), t0Ms: 1000, t1Ms: 5000, cols: [1, 2, 3, 4, 5, 6, 7] },
  title: "Alpha Song",
  artist: "Some Artist",
  version: "Insane",
  creator: "Mapper",
  stars: 4.523,
  level: "dan:7",
  stratum: "dan_07/nps_2",
  played: true,
};

const DETAILS: ChartDetails = {
  md5: "a".repeat(32),
  title: "Alpha Song",
  artist: "Some Artist",
  creator: "Mapper",
  version: "Insane",
  source: "Some Game",
  tags: ["dan", "reform"],
  stars: 4.523,
  od: 8,
  hp: 7.5,
  lengthMs: 154_000,
  bpmMin: 150,
  bpmMax: 180,
  nNotes: 2345,
  nLn: 120,
  setId: 123456,
  beatmapId: 654321,
};

const IMAGE = "data:image/jpeg;base64,/9j/4AAQ";
const COUNTERS = { labelled: 3, skipped: 1, undone: 2, gold: 42 };

function renderHeader(overrides: Partial<ChartHeaderProps> = {}) {
  return render(
    <ChartHeader
      window={WINDOW}
      origin={{ kind: "random" }}
      background={null}
      nav={<button type="button">Next window</button>}
      counters={COUNTERS}
      {...overrides}
    />,
  );
}

/** One stat of the dialog's osu!web-style row, found by its visible caption. */
function stat(dialog: HTMLElement, caption: string): HTMLElement {
  const row = within(dialog).getByRole("list", { name: "Map statistics" });
  const item = within(row)
    .getAllByRole("listitem")
    .find((li) => li.querySelector("[data-stat-caption]")?.textContent === caption);
  if (item === undefined) {
    throw new Error(`no stat ${caption}`);
  }
  return item;
}

async function openDetails(overrides: Partial<ChartHeaderProps> = {}) {
  const user = userEvent.setup();
  renderHeader({ background: IMAGE, details: DETAILS, ...overrides });
  await user.click(screen.getByRole("button", { name: "Show the full image and map details" }));
  return { user, dialog: screen.getByRole("dialog", { name: "Alpha Song" }) };
}

function rgb(hex: string): string {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgb(${String((n >> 16) & 255)}, ${String((n >> 8) & 255)}, ${String(n & 255)})`;
}

describe("ChartHeader", () => {
  it("spans its column edge to edge, as the panel's background, with a separator line under it", () => {
    const { container } = renderHeader({ background: IMAGE });
    const header = container.querySelector("header");
    expect(header?.className).not.toMatch(/(^|\s)(m|mx|ml|mr|p|px|pl|pr)-\S+/);
    expect(header?.className).not.toMatch(/(^|\s)border(\s|$)/);
    expect(header).toHaveClass("w-full", "border-b");
  });

  it("names the chart: title as a heading, artist, mapper, difficulty name, origin and tags", () => {
    renderHeader();
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();
    expect(screen.getByText("Some Artist")).toBeInTheDocument();
    expect(screen.getByText("mapped by Mapper")).toBeInTheDocument();
    expect(screen.getByText("Insane")).toBeInTheDocument();
    expect(screen.getByText("Random pick")).toBeInTheDocument();
    expect(screen.getByText("Level dan:7")).toBeInTheDocument();
    expect(screen.getByText("Stratum dan_07/nps_2")).toBeInTheDocument();
    expect(screen.getByText("Played")).toBeInTheDocument();
  });

  it("shows the star rating to two decimals on osu!'s difficulty colour, with dark text below 6.5", () => {
    renderHeader();
    const badge = screen.getByTestId("star-rating");
    expect(badge).toHaveTextContent("4.52");
    expect(badge).toHaveAccessibleName("4.52 stars");
    // 4.523 sits between the 4.2 (#FF8068) and 4.9 (#FF4E6F) stops.
    expect(badge.style.backgroundColor).toMatch(/^rgb\(255, \d+, \d+\)$/);
    expect(badge.style.color).toBe("rgba(0, 0, 0, 0.75)");
  });

  it("writes the star rating in osu!'s gold on the darkest colours", () => {
    renderHeader({ window: { ...WINDOW, stars: 7.7 } });
    const badge = screen.getByTestId("star-rating");
    expect(badge.style.backgroundColor).toBe(rgb("#18158E"));
    expect(badge.style.color).toBe(rgb("#FFD966"));
  });

  it("shows no star rating when stable has not computed one", () => {
    renderHeader({ window: { ...WINDOW, stars: null } });
    expect(screen.queryByTestId("star-rating")).toBeNull();
  });

  it("works without an origin", () => {
    renderHeader({ origin: undefined });
    expect(screen.queryByText("Random pick")).toBeNull();
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();
  });
});

describe("ChartHeader background", () => {
  it("stacks a sharp image under a blurred copy masked in from the top, both decorative", () => {
    renderHeader({ background: IMAGE });
    const sharp = screen.getByTestId("header-bg-sharp");
    const blurred = screen.getByTestId("header-bg-blur");
    for (const img of [sharp, blurred]) {
      expect(img).toHaveAttribute("src", IMAGE);
      expect(img).toHaveAttribute("alt", "");
    }
    expect(sharp.className).not.toMatch(/\bblur/);
    expect(blurred.className).toMatch(/\bblur-/);
    expect(blurred.className).toMatch(/mask-image:linear-gradient\(to_bottom,transparent/);
    expect(screen.queryByTestId("header-fallback")).toBeNull();
  });

  it("darkens towards the text so it keeps its contrast", () => {
    renderHeader({ background: IMAGE });
    expect(screen.getByTestId("header-scrim").className).toMatch(/to-background\b/);
  });

  it("falls back to a gradient without a background, keeping the same box", () => {
    const { container } = renderHeader({ background: null });
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByTestId("header-fallback")).toBeInTheDocument();
    const withImage = renderHeader({ background: IMAGE });
    const classes = [container, withImage.container].map((c) => c.querySelector("header")?.className);
    expect(classes[0]).toBeDefined();
    expect(classes[0]).toBe(classes[1]);
  });

  it("builds the data URL from the background command's payload", () => {
    expect(backgroundDataUrl({ mime: "image/png", base64: "iVBO" })).toBe("data:image/png;base64,iVBO");
    expect(backgroundDataUrl(null)).toBeNull();
    expect(backgroundDataUrl(undefined)).toBeNull();
  });
});

describe("ChartHeader card contents", () => {
  it("renders the navigation slot inside the card", () => {
    const { container } = renderHeader();
    const header = container.querySelector("header");
    expect(header).not.toBeNull();
    expect(within(header ?? document.body).getByRole("button", { name: "Next window" })).toBeInTheDocument();
  });

  it("shows the session counters compactly, each spelled out for assistive technology", () => {
    renderHeader();
    const list = screen.getByRole("list", { name: "Session" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(4);
    for (const text of ["Labelled: 3", "Skipped: 1", "Undone: 2", "Gold set: 42"]) {
      expect(within(list).getByText(text)).toBeInTheDocument();
    }
  });
});

describe("ChartHeader details dialog", () => {
  it("lists each tag once, in first-seen order", async () => {
    const user = userEvent.setup();
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    renderHeader({ details: { ...DETAILS, tags: ["dan", "reform", "dan", "7k", "reform"] } });

    await user.click(screen.getByRole("button", { name: "Show the full image and map details" }));

    const dialog = screen.getByRole("dialog", { name: "Alpha Song" });
    const tags = within(dialog).getByText("Tags", { selector: "dt" }).nextElementSibling as HTMLElement;
    expect(within(tags).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["dan", "reform", "7k"]);
    // React reports duplicate keys on console.error.
    expect(errors).not.toHaveBeenCalled();
    errors.mockRestore();
  });

  it("opens from the corner button with the full image and every detail, then returns focus on close", async () => {
    const user = userEvent.setup();
    renderHeader({ background: IMAGE, details: DETAILS });
    const trigger = screen.getByRole("button", { name: "Show the full image and map details" });

    await user.click(trigger);

    const dialog = screen.getByRole("dialog", { name: "Alpha Song" });
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
    const image = within(dialog).getByRole("img", { name: "Background of Alpha Song" });
    expect(image).toHaveAttribute("src", IMAGE);
    expect(image).toHaveClass("object-contain");

    const term = (name: string): HTMLElement => {
      const dt = within(dialog).getByText(name, { selector: "dt" });
      const dd = dt.nextElementSibling;
      expect(dd?.tagName).toBe("DD");
      return dd as HTMLElement;
    };
    expect(term("Mapper")).toHaveTextContent("Mapper");
    expect(term("Difficulty")).toHaveTextContent("Insane");
    expect(within(term("Star rating")).getByTestId("star-rating")).toHaveTextContent("4.52");
    expect(term("OD")).toHaveTextContent("8");
    expect(term("HP")).toHaveTextContent("7.5");
    expect(term("Source")).toHaveTextContent("Some Game");
    expect(within(term("Tags")).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["dan", "reform"]);
    expect(term("Beatmap set ID")).toHaveTextContent("123456");
    expect(term("Beatmap ID")).toHaveTextContent("654321");

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(trigger).toHaveFocus();
  });

  it("shows a single BPM when the chart has one tempo and skips missing fields", async () => {
    const user = userEvent.setup();
    renderHeader({
      details: {
        ...DETAILS,
        bpmMin: 180,
        bpmMax: 180,
        source: null,
        tags: [],
        setId: null,
        beatmapId: null,
        od: null,
        hp: null,
      },
    });

    await user.click(screen.getByRole("button", { name: "Show the full image and map details" }));

    const dialog = screen.getByRole("dialog");
    expect(stat(dialog, "BPM")).toHaveTextContent(/^180BPM$/);
    for (const name of ["Source", "Tags", "Beatmap set ID", "Beatmap ID", "OD", "HP"]) {
      expect(within(dialog).queryByText(name, { selector: "dt" })).toBeNull();
    }
    expect(within(dialog).queryByRole("img", { name: /Background of/ })).toBeNull();
    expect(within(dialog).queryByRole("button", { name: "Open on osu!" })).toBeNull();
  });

  it("falls back to the window's metadata before the details arrive", async () => {
    const user = userEvent.setup();
    renderHeader();

    await user.click(screen.getByRole("button", { name: "Show the full image and map details" }));

    const dialog = screen.getByRole("dialog", { name: "Alpha Song" });
    expect(within(dialog).getByText("Difficulty", { selector: "dt" }).nextElementSibling).toHaveTextContent("Insane");
    expect(within(dialog).queryByRole("list", { name: "Map statistics" })).toBeNull();
  });

  it("names the dialog's close button in the interface language", async () => {
    const user = userEvent.setup();
    renderHeader();
    await user.click(screen.getByRole("button", { name: "Show the full image and map details" }));

    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Close" }));

    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("ChartHeader details dialog layout", () => {
  it("leads with osu!web's stat row: length, BPM, notes and long notes, each iconed in osu! yellow and captioned", async () => {
    const { dialog } = await openDetails();
    const row = within(dialog).getByRole("list", { name: "Map statistics" });
    expect(within(row).getAllByRole("listitem")).toHaveLength(4);
    expect(stat(dialog, "Length")).toHaveTextContent("02:34");
    expect(stat(dialog, "BPM")).toHaveTextContent("150–180");
    expect(stat(dialog, "Notes")).toHaveTextContent("2,345");
    expect(stat(dialog, "Long notes")).toHaveTextContent("120");
    for (const item of within(row).getAllByRole("listitem")) {
      const icon = item.querySelector("svg");
      expect(icon).toHaveAttribute("aria-hidden", "true");
      expect(icon?.getAttribute("class")).toMatch(/\btext-osu-yellow\b/);
    }
    // The image sits above the stats, the definition grid below them.
    const image = within(dialog).getByRole("img", { name: "Background of Alpha Song" });
    const grid = within(dialog).getByText("Mapper", { selector: "dt" }).closest("dl");
    expect(image.compareDocumentPosition(row) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(row.compareDocumentPosition(grid as Node) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("is wide and lays the details out in two columns, tags as chips and the MD5 in monospace", async () => {
    const { dialog } = await openDetails();
    expect(dialog).toHaveClass("sm:max-w-3xl");
    const grid = within(dialog).getByText("Mapper", { selector: "dt" }).closest("dl");
    expect(grid).toHaveClass("sm:grid-cols-2");
    const tags = within(dialog).getByText("Tags", { selector: "dt" }).nextElementSibling as HTMLElement;
    for (const chip of within(tags).getAllByRole("listitem")) {
      expect(chip).toHaveClass("rounded-full");
    }
    expect(within(dialog).getByText(DETAILS.md5)).toHaveClass("font-mono");
  });

  it("copies the MD5 and says so", async () => {
    const { user, dialog } = await openDetails();
    await user.click(within(dialog).getByRole("button", { name: "Copy MD5" }));
    await expect(navigator.clipboard.readText()).resolves.toBe(DETAILS.md5);
    expect(within(dialog).getByRole("status")).toHaveTextContent("MD5 copied");
  });

  it("opens the difficulty on osu.ppy.sh through the opener plugin", async () => {
    const calls = mockCommands({}, { "plugin:opener|open_url": () => null });
    const { user, dialog } = await openDetails();
    await user.click(within(dialog).getByRole("button", { name: "Open on osu!" }));
    await waitFor(() => {
      expect(calls.filter((c) => c.cmd === "plugin:opener|open_url").map((c) => c.args["url"])).toEqual([
        "https://osu.ppy.sh/beatmapsets/123456#mania/654321",
      ]);
    });
  });

  it("opens the beatmap set when the difficulty has no ID, and reports a failure to open", async () => {
    const calls = mockCommands(
      {},
      {
        "plugin:opener|open_url": () => {
          throw new Error("not allowed");
        },
      },
    );
    const { user, dialog } = await openDetails({ details: { ...DETAILS, beatmapId: null } });
    await user.click(within(dialog).getByRole("button", { name: "Open on osu!" }));
    await waitFor(() => {
      expect(within(dialog).getByRole("alert")).toHaveTextContent("Could not open the osu! website");
    });
    expect(calls.find((c) => c.cmd === "plugin:opener|open_url")?.args["url"]).toBe("https://osu.ppy.sh/beatmapsets/123456");
  });
});

interface ObserverCall {
  callback: IntersectionObserverCallback;
  options: IntersectionObserverInit | undefined;
  targets: Element[];
}

function stubIntersectionObserver(): ObserverCall[] {
  const observers: ObserverCall[] = [];
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      private readonly call: ObserverCall;
      constructor(callback: IntersectionObserverCallback, options?: IntersectionObserverInit) {
        this.call = { callback, options, targets: [] };
        observers.push(this.call);
      }
      observe(target: Element): void {
        this.call.targets.push(target);
      }
      unobserve(): void {
        // Nothing is held.
      }
      disconnect(): void {
        // Nothing is held.
      }
    },
  );
  return observers;
}

function report(observer: ObserverCall | undefined, isIntersecting: boolean) {
  act(() => {
    observer?.callback(
      observer.targets.map((target) => ({ target, isIntersecting }) as unknown as IntersectionObserverEntry),
      observer as unknown as IntersectionObserver,
    );
  });
}

describe("ChartHeader compact bar", () => {
  function renderInPanel(overrides: Partial<ChartHeaderProps> = {}) {
    const root = document.createElement("div");
    document.body.append(root);
    const scrollRoot = createRef<HTMLElement>();
    (scrollRoot as { current: HTMLElement | null }).current = root;
    const view = renderHeader({ background: IMAGE, scrollRoot, origin: { kind: "plan", round: 0 }, ...overrides });
    return { view, root };
  }

  it("stays out of the way while the card is in view", () => {
    stubIntersectionObserver();
    renderInPanel();
    expect(screen.queryByTestId("header-compact")).toBeNull();
  });

  it("watches the card's title against the panel's top edge, under the bar's own height", () => {
    const observers = stubIntersectionObserver();
    const { root } = renderInPanel();
    expect(observers).toHaveLength(1);
    expect(observers[0]?.options?.root).toBe(root);
    expect(observers[0]?.options?.rootMargin).toBe(`-${String(CHART_HEADER_PARAMS.compactBarPx)}px 0px 0px 0px`);
    expect(observers[0]?.targets).toEqual([screen.getByTestId("header-sentinel")]);
    expect(screen.getByTestId("header-sentinel").closest("header")).not.toBeNull();
  });

  it("collapses into a sticky title bar once the card scrolls away, with the round badge moved into it", () => {
    const observers = stubIntersectionObserver();
    renderInPanel();
    report(observers[0], false);

    const bar = screen.getByTestId("header-compact");
    expect(bar.parentElement).toHaveClass("sticky", "top-0");
    expect(bar).toHaveStyle({ height: `${String(CHART_HEADER_PARAMS.compactBarPx)}px` });
    expect(bar.getAttribute("class")).toMatch(/motion-safe:animate-in/);
    expect(bar.getAttribute("class")).not.toMatch(/(^|\s)animate-in/);
    expect(within(bar).getByText("Alpha Song")).toBeInTheDocument();
    expect(within(bar).queryByRole("heading")).toBeNull();
    expect(within(bar).getByTestId("star-rating")).toHaveTextContent("4.52");
    expect(within(bar).getByText("Round 1")).toBeInTheDocument();
    expect(within(bar).getByTestId("header-compact-thumb")).toHaveAttribute("src", IMAGE);
    expect(within(bar).getByTestId("header-compact-thumb")).toHaveAttribute("alt", "");
    expect(within(bar).getByRole("button", { name: "Show the full image and map details" })).toBeInTheDocument();

    report(observers[0], true);
    expect(screen.queryByTestId("header-compact")).toBeNull();
  });

  it("carries the navigation into the bar, so Previous, Random and Now playing stay in reach while scrolled", () => {
    const observers = stubIntersectionObserver();
    renderInPanel();
    report(observers[0], false);
    expect(within(screen.getByTestId("header-compact")).getByRole("button", { name: "Next window" })).toBeInTheDocument();
  });

  it("takes the scrolled-away card's controls out of reach while collapsed, so each one exists once", () => {
    const observers = stubIntersectionObserver();
    const { view } = renderInPanel();
    const header = view.container.querySelector("header");
    if (header === null) {
      throw new Error("no card");
    }
    const cardControls = (): HTMLElement[] =>
      within(header).getAllByRole("button", { name: /Next window|Show the full image and map details/ });
    expect(cardControls().every((b) => b.closest("[inert]") === null)).toBe(true);

    report(observers[0], false);
    expect(cardControls()).toHaveLength(2);
    expect(cardControls().every((b) => b.closest("[inert]") !== null)).toBe(true);
    expect(within(header).getByText("Round 1").closest("[inert]")).not.toBeNull();
  });

  it("collapses only once the card's navigation row has passed under the bar, so no visible control is inert", () => {
    stubIntersectionObserver();
    const { view } = renderInPanel();
    const header = view.container.querySelector("header");
    expect(header?.lastElementChild).toBe(screen.getByTestId("header-sentinel"));
  });

  it("opens the details from the compact bar too", async () => {
    const user = userEvent.setup();
    const observers = stubIntersectionObserver();
    renderInPanel({ details: DETAILS });
    report(observers[0], false);
    await user.click(within(screen.getByTestId("header-compact")).getByRole("button", { name: "Show the full image and map details" }));
    expect(screen.getByRole("dialog", { name: "Alpha Song" })).toBeInTheDocument();
  });

  it("keeps the full card where the browser has no IntersectionObserver", () => {
    vi.stubGlobal("IntersectionObserver", undefined);
    renderInPanel();
    expect(screen.queryByTestId("header-compact")).toBeNull();
    expect(screen.getByRole("heading", { name: "Alpha Song" })).toBeInTheDocument();
  });

  it("reports the card's height, so the panel can paint its background under the scrollbar", () => {
    const sizes: number[] = [];
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(private readonly onResize: ResizeObserverCallback) {}
        observe(target: Element): void {
          this.onResize([{ target, borderBoxSize: [{ blockSize: 212, inlineSize: 300 }] } as unknown as ResizeObserverEntry], this);
        }
        unobserve(): void {
          // Nothing is held.
        }
        disconnect(): void {
          // Nothing is held.
        }
      },
    );
    renderHeader({ onBlockSize: (px) => sizes.push(px) });
    expect(sizes).toEqual([212]);
  });
});
