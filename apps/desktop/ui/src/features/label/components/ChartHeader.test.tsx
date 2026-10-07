import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { LabelWindow } from "../types";
import { ChartHeader, type ChartHeaderProps } from "./ChartHeader";

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

function renderHeader(overrides: Partial<ChartHeaderProps> = {}) {
  return render(<ChartHeader window={WINDOW} origin={{ kind: "random" }} background={null} {...overrides} />);
}

function rgb(hex: string): string {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgb(${String((n >> 16) & 255)}, ${String((n >> 8) & 255)}, ${String(n & 255)})`;
}

describe("ChartHeader", () => {
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

  it("puts the chart's background behind the text as a decorative data URL image", () => {
    const { container } = renderHeader({ background: { mime: "image/jpeg", base64: "/9j/4AAQ" } });
    const img = container.querySelector("img");
    expect(img).not.toBeNull();
    expect(img).toHaveAttribute("src", "data:image/jpeg;base64,/9j/4AAQ");
    expect(img).toHaveAttribute("alt", "");
    expect(screen.queryByTestId("header-fallback")).toBeNull();
  });

  it("falls back to a gradient without a background, keeping the same fixed height", () => {
    const { container } = renderHeader({ background: null });
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByTestId("header-fallback")).toBeInTheDocument();
    const withImage = render(
      <ChartHeader window={WINDOW} origin={{ kind: "random" }} background={{ mime: "image/png", base64: "iVBO" }} />,
    );
    const heights = [container, withImage.container].map((c) => c.querySelector("header")?.className.match(/\bh-\d+\b/)?.[0]);
    expect(heights[0]).toBeDefined();
    expect(heights[0]).toBe(heights[1]);
  });
});
