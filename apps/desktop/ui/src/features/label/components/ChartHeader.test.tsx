import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { LabelWindow } from "../types";
import { backgroundDataUrl, type ChartDetails, ChartHeader, type ChartHeaderProps } from "./ChartHeader";

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
    expect(term("Length")).toHaveTextContent("02:34");
    expect(term("BPM")).toHaveTextContent("150–180");
    expect(term("OD")).toHaveTextContent("8");
    expect(term("HP")).toHaveTextContent("7.5");
    expect(term("Notes")).toHaveTextContent("2345");
    expect(term("Long notes")).toHaveTextContent("120");
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
    const bpm = within(dialog).getByText("BPM", { selector: "dt" }).nextElementSibling;
    expect(bpm).toHaveTextContent(/^180$/);
    for (const name of ["Source", "Tags", "Beatmap set ID", "Beatmap ID", "OD", "HP"]) {
      expect(within(dialog).queryByText(name, { selector: "dt" })).toBeNull();
    }
    expect(within(dialog).queryByRole("img", { name: /Background of/ })).toBeNull();
  });

  it("falls back to the window's metadata before the details arrive", async () => {
    const user = userEvent.setup();
    renderHeader();

    await user.click(screen.getByRole("button", { name: "Show the full image and map details" }));

    const dialog = screen.getByRole("dialog", { name: "Alpha Song" });
    expect(within(dialog).getByText("Difficulty", { selector: "dt" }).nextElementSibling).toHaveTextContent("Insane");
    expect(within(dialog).queryByText("Length", { selector: "dt" })).toBeNull();
  });

  it("names the dialog's close button in the interface language", async () => {
    const user = userEvent.setup();
    renderHeader();
    await user.click(screen.getByRole("button", { name: "Show the full image and map details" }));

    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Close" }));

    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
