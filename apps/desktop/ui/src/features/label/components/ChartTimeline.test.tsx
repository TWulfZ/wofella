import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ChartTimeline, type ChartTimelineProps } from "./ChartTimeline";

// A 100 s chart drawn 1000 px wide from x = 100, so 1 px is 100 ms and x = 100 + t / 100.
const TRACK = { left: 100, width: 1000 };

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: TRACK.left,
    y: 0,
    left: TRACK.left,
    top: 0,
    width: TRACK.width,
    height: 40,
    right: TRACK.left + TRACK.width,
    bottom: 40,
    toJSON: () => ({}),
  });
});

function renderTimeline(overrides: Partial<ChartTimelineProps> = {}) {
  const onMove = vi.fn();
  const props: ChartTimelineProps = {
    span: { firstMs: 0, endMs: 100_000 },
    window: { t0Ms: 20_000, t1Ms: 30_000 },
    density: [0, 4, 8, 2],
    labelled: [],
    locked: false,
    onMove,
    ...overrides,
  };
  const view = render(<ChartTimeline {...props} />);
  return { onMove, view, props };
}

function slider(): HTMLElement {
  return screen.getByRole("slider", { name: "Window position" });
}

function track(): HTMLElement {
  return screen.getByTestId("timeline-track");
}

describe("ChartTimeline", () => {
  it("is a slider over the window start, bounded so the window stays inside the chart, voiced in mm:ss", () => {
    renderTimeline();
    expect(screen.getByRole("group", { name: "Chart timeline" })).toContainElement(slider());
    expect(slider()).toHaveAttribute("aria-valuemin", "0");
    expect(slider()).toHaveAttribute("aria-valuemax", "90000");
    expect(slider()).toHaveAttribute("aria-valuenow", "20000");
    expect(slider()).toHaveAttribute("aria-valuetext", "00:20 to 00:30");
  });

  it("draws the current window as a range over the chart span", () => {
    renderTimeline();
    expect(slider().style.left).toBe("20%");
    expect(slider().style.width).toBe("10%");
  });

  it("shades the viewer's own labelled spans", () => {
    renderTimeline({ labelled: [{ t0Ms: 50_000, t1Ms: 55_000 }, { t0Ms: 70_000, t1Ms: 80_000 }] });
    const spans = screen.getAllByTestId("timeline-labelled");
    expect(spans.map((s) => [s.style.left, s.style.width])).toEqual([
      ["50%", "5%"],
      ["70%", "10%"],
    ]);
    expect(screen.getByText("2 labelled windows on this chart")).toBeInTheDocument();
  });

  it("draws one density bar per bucket, scaled to the busiest", () => {
    renderTimeline();
    const bars = screen.getAllByTestId("timeline-density-bar");
    expect(bars.map((b) => b.getAttribute("height"))).toEqual(["0", "0.5", "1", "0.25"]);
  });

  it("draws no density bars while the density is unknown", () => {
    renderTimeline({ density: null });
    expect(screen.queryAllByTestId("timeline-density-bar")).toEqual([]);
  });

  it("steps by 1 s with the arrows and 5 s with Page Up/Down, and jumps with Home/End", async () => {
    const { onMove, view, props } = renderTimeline();
    slider().focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(onMove).toHaveBeenLastCalledWith(21_000);
    await userEvent.keyboard("{ArrowLeft}");
    expect(onMove).toHaveBeenLastCalledWith(19_000);
    await userEvent.keyboard("{PageUp}");
    expect(onMove).toHaveBeenLastCalledWith(25_000);
    await userEvent.keyboard("{PageDown}");
    expect(onMove).toHaveBeenLastCalledWith(15_000);
    await userEvent.keyboard("{Home}");
    expect(onMove).toHaveBeenLastCalledWith(0);
    await userEvent.keyboard("{End}");
    expect(onMove).toHaveBeenLastCalledWith(90_000);

    // At an end a step that would leave the chart does nothing.
    onMove.mockClear();
    view.rerender(<ChartTimeline {...props} window={{ t0Ms: 90_000, t1Ms: 100_000 }} />);
    await userEvent.keyboard("{ArrowRight}{PageUp}");
    expect(onMove).not.toHaveBeenCalled();
  });

  it("centres the window on a click outside it, committing on pointer up", () => {
    const { onMove } = renderTimeline();
    fireEvent.pointerDown(track(), { clientX: 100 + 600, button: 0, pointerId: 1 });
    expect(onMove).not.toHaveBeenCalled();
    expect(slider()).toHaveAttribute("aria-valuenow", "55000");
    fireEvent.pointerUp(window, { clientX: 100 + 600, pointerId: 1 });
    expect(onMove).toHaveBeenCalledExactlyOnceWith(55_000);
  });

  it("drags the window by where it was grabbed, clamped inside the chart, committing once", () => {
    const { onMove } = renderTimeline();
    // Grabbed 2 s into the window.
    fireEvent.pointerDown(slider(), { clientX: 100 + 220, button: 0, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 100 + 420, pointerId: 1 });
    expect(slider()).toHaveAttribute("aria-valuenow", "40000");
    expect(slider().style.left).toBe("40%");
    fireEvent.pointerMove(window, { clientX: 100 + 2000, pointerId: 1 });
    expect(slider()).toHaveAttribute("aria-valuenow", "90000");
    expect(onMove).not.toHaveBeenCalled();
    fireEvent.pointerUp(window, { clientX: 100 + 2000, pointerId: 1 });
    expect(onMove).toHaveBeenCalledExactlyOnceWith(90_000);
    fireEvent.pointerMove(window, { clientX: 100, pointerId: 1 });
    expect(onMove).toHaveBeenCalledTimes(1);
  });

  it("commits nothing when a drag ends where it started", () => {
    const { onMove } = renderTimeline();
    fireEvent.pointerDown(slider(), { clientX: 100 + 250, button: 0, pointerId: 1 });
    fireEvent.pointerUp(window, { clientX: 100 + 250, pointerId: 1 });
    expect(onMove).not.toHaveBeenCalled();
  });

  it("ignores pointer and keys on a locked (saved) window, and says why", async () => {
    const { onMove } = renderTimeline({ locked: true });
    expect(slider()).toHaveAttribute("aria-disabled", "true");
    expect(slider()).toHaveAccessibleDescription("Undo the label to move a saved window.");
    fireEvent.pointerDown(track(), { clientX: 100 + 600, button: 0, pointerId: 1 });
    fireEvent.pointerUp(window, { clientX: 100 + 600, pointerId: 1 });
    slider().focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(onMove).not.toHaveBeenCalled();
    expect(slider()).toHaveAttribute("aria-valuenow", "20000");
  });

  it("ignores buttons other than the primary one", () => {
    const { onMove } = renderTimeline();
    fireEvent.pointerDown(track(), { clientX: 100 + 600, button: 2, pointerId: 1 });
    fireEvent.pointerUp(window, { clientX: 100 + 600, pointerId: 1 });
    expect(onMove).not.toHaveBeenCalled();
  });
});
