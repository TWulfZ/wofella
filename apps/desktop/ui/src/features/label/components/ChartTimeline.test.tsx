import { act, fireEvent, render, screen } from "@testing-library/react";
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
  const onResize = vi.fn();
  const props: ChartTimelineProps = {
    span: { firstMs: 0, endMs: 100_000 },
    window: { t0Ms: 20_000, t1Ms: 30_000 },
    density: [0, 4, 8, 2],
    labelled: [],
    locked: false,
    onMove,
    onResize,
    ...overrides,
  };
  const view = render(<ChartTimeline {...props} />);
  return { onMove, onResize, view, props };
}

function slider(): HTMLElement {
  return screen.getByRole("slider", { name: "Window position" });
}

function startHandle(): HTMLElement {
  return screen.getByRole("slider", { name: "Window start" });
}

function endHandle(): HTMLElement {
  return screen.getByRole("slider", { name: "Window end" });
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

  describe("resize handles", () => {
    it("sit on both edges of the window as sliders over that edge, bounded to 1..60 s and the chart", () => {
      renderTimeline();
      expect(startHandle()).toHaveAttribute("aria-valuenow", "20000");
      expect(startHandle()).toHaveAttribute("aria-valuemin", "0");
      expect(startHandle()).toHaveAttribute("aria-valuemax", "29000");
      expect(startHandle()).toHaveAttribute("aria-valuetext", "00:20, window 10.0 s");
      expect(endHandle()).toHaveAttribute("aria-valuenow", "30000");
      expect(endHandle()).toHaveAttribute("aria-valuemin", "21000");
      expect(endHandle()).toHaveAttribute("aria-valuemax", "80000");
      expect(startHandle().style.left).toBe("20%");
      expect(endHandle().style.left).toBe("30%");
    });

    it("cap the far edge at the chart end", () => {
      renderTimeline({ window: { t0Ms: 70_000, t1Ms: 80_000 } });
      expect(endHandle()).toHaveAttribute("aria-valuemax", "100000");
      expect(startHandle()).toHaveAttribute("aria-valuemin", "20000");
    });

    it("step the focused edge by 1 s with the arrows and 5 s with Page Up/Down", async () => {
      const { onResize, onMove } = renderTimeline();
      startHandle().focus();
      await userEvent.keyboard("{ArrowLeft}");
      expect(onResize).toHaveBeenLastCalledWith({ t0Ms: 19_000, t1Ms: 30_000 });
      await userEvent.keyboard("{PageUp}");
      expect(onResize).toHaveBeenLastCalledWith({ t0Ms: 25_000, t1Ms: 30_000 });
      endHandle().focus();
      await userEvent.keyboard("{ArrowRight}");
      expect(onResize).toHaveBeenLastCalledWith({ t0Ms: 20_000, t1Ms: 31_000 });
      await userEvent.keyboard("{PageDown}");
      expect(onResize).toHaveBeenLastCalledWith({ t0Ms: 20_000, t1Ms: 25_000 });
      expect(onMove).not.toHaveBeenCalled();
    });

    it("never shrink below 1 s or grow past 60 s", async () => {
      const { onResize, view, props } = renderTimeline({ window: { t0Ms: 20_000, t1Ms: 21_000 } });
      endHandle().focus();
      await userEvent.keyboard("{ArrowLeft}{PageDown}");
      startHandle().focus();
      await userEvent.keyboard("{ArrowRight}");
      expect(onResize).not.toHaveBeenCalled();

      view.rerender(<ChartTimeline {...props} window={{ t0Ms: 20_000, t1Ms: 79_000 }} />);
      endHandle().focus();
      await userEvent.keyboard("{PageUp}");
      expect(onResize).toHaveBeenLastCalledWith({ t0Ms: 20_000, t1Ms: 80_000 });
    });

    it("drag an edge with the pointer, keeping the other one, committing once on release", () => {
      const { onResize, onMove } = renderTimeline();
      fireEvent.pointerDown(endHandle(), { clientX: 100 + 300, button: 0, pointerId: 1 });
      fireEvent.pointerMove(window, { clientX: 100 + 450, pointerId: 1 });
      expect(endHandle()).toHaveAttribute("aria-valuenow", "45000");
      expect(slider().style.width).toBe("25%");
      // Past 60 s the edge stops.
      fireEvent.pointerMove(window, { clientX: 100 + 950, pointerId: 1 });
      expect(endHandle()).toHaveAttribute("aria-valuenow", "80000");
      expect(onResize).not.toHaveBeenCalled();
      fireEvent.pointerUp(window, { clientX: 100 + 950, pointerId: 1 });
      expect(onResize).toHaveBeenCalledExactlyOnceWith({ t0Ms: 20_000, t1Ms: 80_000 });
      expect(onMove).not.toHaveBeenCalled();
    });

    it("drag the start edge, never past 1 s before the end", () => {
      const { onResize } = renderTimeline();
      fireEvent.pointerDown(startHandle(), { clientX: 100 + 200, button: 0, pointerId: 1 });
      fireEvent.pointerMove(window, { clientX: 100 + 350, pointerId: 1 });
      expect(startHandle()).toHaveAttribute("aria-valuenow", "29000");
      fireEvent.pointerUp(window, { clientX: 100 + 350, pointerId: 1 });
      expect(onResize).toHaveBeenCalledExactlyOnceWith({ t0Ms: 29_000, t1Ms: 30_000 });
    });

    describe("on a window a few px wide", () => {
      // 300 s over 900 px from x = 0 is 3 px a second: the 4 s window from 150 s spans x = 450..462.
      const LONG_CHART: Partial<ChartTimelineProps> = {
        span: { firstMs: 0, endMs: 300_000 },
        window: { t0Ms: 150_000, t1Ms: 154_000 },
      };
      beforeEach(() => {
        vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
          x: 0,
          y: 0,
          left: 0,
          top: 0,
          width: 900,
          height: 40,
          right: 900,
          bottom: 40,
          toJSON: () => ({}),
        });
      });

      it("moves the window when its middle is dragged, even where a handle's box overlaps it", () => {
        const { onMove, onResize } = renderTimeline(LONG_CHART);
        // The browser hands the press to whichever element's box is on top; a handle's box may cover the body.
        fireEvent.pointerDown(endHandle(), { clientX: 456, button: 0, pointerId: 1 });
        fireEvent.pointerMove(window, { clientX: 486, pointerId: 1 });
        fireEvent.pointerUp(window, { clientX: 486, pointerId: 1 });

        expect(onMove).toHaveBeenCalledExactlyOnceWith(160_000);
        expect(onResize).not.toHaveBeenCalled();
      });

      it("keeps a grab area at least 16 px wide for the move", () => {
        const { onMove, onResize } = renderTimeline(LONG_CHART);
        fireEvent.pointerDown(track(), { clientX: 449, button: 0, pointerId: 1 });
        fireEvent.pointerMove(window, { clientX: 479, pointerId: 1 });
        fireEvent.pointerUp(window, { clientX: 479, pointerId: 1 });

        expect(onMove).toHaveBeenCalledExactlyOnceWith(160_000);
        expect(onResize).not.toHaveBeenCalled();
      });

      it("resizes from a press just outside either edge, keeping the edge's offset from the pointer", () => {
        const { onMove, onResize } = renderTimeline(LONG_CHART);
        fireEvent.pointerDown(track(), { clientX: 468, button: 0, pointerId: 1 });
        fireEvent.pointerMove(window, { clientX: 492, pointerId: 1 });
        fireEvent.pointerUp(window, { clientX: 492, pointerId: 1 });
        expect(onResize).toHaveBeenLastCalledWith({ t0Ms: 150_000, t1Ms: 162_000 });

        fireEvent.pointerDown(track(), { clientX: 444, button: 0, pointerId: 1 });
        fireEvent.pointerMove(window, { clientX: 420, pointerId: 1 });
        fireEvent.pointerUp(window, { clientX: 420, pointerId: 1 });
        expect(onResize).toHaveBeenLastCalledWith({ t0Ms: 142_000, t1Ms: 154_000 });
        expect(onMove).not.toHaveBeenCalled();
      });

      it("draws the handles outside the window, leaving its body uncovered", () => {
        renderTimeline(LONG_CHART);
        expect(startHandle()).toHaveClass("-translate-x-full");
        expect(endHandle()).not.toHaveClass("-translate-x-1/2");
        expect(endHandle()).not.toHaveClass("-translate-x-full");
      });
    });

    it("do nothing on a locked (saved) window", async () => {
      const { onResize } = renderTimeline({ locked: true });
      expect(startHandle()).toHaveAttribute("aria-disabled", "true");
      expect(endHandle()).toHaveAccessibleDescription("Undo the label to move a saved window.");
      fireEvent.pointerDown(endHandle(), { clientX: 100 + 300, button: 0, pointerId: 1 });
      fireEvent.pointerUp(window, { clientX: 100 + 500, pointerId: 1 });
      endHandle().focus();
      await userEvent.keyboard("{ArrowRight}");
      expect(onResize).not.toHaveBeenCalled();
    });
  });
});

describe("ChartTimeline playhead", () => {
  // The section plays 19.0–30.25 s (pre- and post-roll around the 20–30 s window): 1 px of the seek bar is 11.25 ms.
  const LOOP = { startMs: 19_000, endMs: 30_250 };

  function renderPlayhead(positionMs = 21_250, overrides: Partial<ChartTimelineProps> = {}) {
    const position = { ms: positionMs };
    const onSeek = vi.fn((ms: number) => {
      position.ms = ms;
    });
    const rendered = renderTimeline({
      playhead: { positionMs: () => position.ms, loop: LOOP, stepMs: 2000, onSeek },
      ...overrides,
    });
    return { ...rendered, onSeek, position };
  }

  function playhead(): HTMLElement {
    return screen.getByRole("slider", { name: "Playback position" });
  }

  function seekBar(): HTMLElement {
    return screen.getByTestId("seek-bar");
  }

  it("is a slider over the playing section, voiced as mm:ss.mmm", () => {
    renderPlayhead();
    expect(screen.getByRole("group", { name: "Chart timeline" })).toContainElement(playhead());
    expect(playhead()).toHaveAttribute("aria-valuemin", "19000");
    expect(playhead()).toHaveAttribute("aria-valuemax", "30250");
    expect(playhead()).toHaveAttribute("aria-valuenow", "21250");
    expect(playhead()).toHaveAttribute("aria-valuetext", "00:21.250");
    expect(playhead()).toHaveAttribute("tabindex", "0");
  });

  it("marks the position on the seek bar, which spans the section, and as a line inside the chart's window", () => {
    renderPlayhead();
    expect(playhead().style.left).toBe("20%");
    expect(screen.getByTestId("seek-window").style.left).toBe(`${String(Math.round((1000 / 11_250) * 10_000) / 100)}%`);
    expect(screen.getByTestId("timeline-playhead-line").style.left).toBe("21.25%");
    expect(screen.getByTestId("timeline-playhead-line")).toHaveAttribute("aria-hidden", "true");
  });

  it("follows the playback position frame by frame", async () => {
    const { position } = renderPlayhead();
    position.ms = 24_625;
    await act(async () => {
      await new Promise((resolve) => requestAnimationFrame(resolve));
      await new Promise((resolve) => requestAnimationFrame(resolve));
    });
    expect(playhead().style.left).toBe("50%");
  });

  it("seeks where the seek bar is clicked, without moving the window", () => {
    const { onSeek, onMove, onResize } = renderPlayhead();
    // x = 100 + 400 px → 19 000 + 400 × 11.25 ms.
    fireEvent.pointerDown(seekBar(), { button: 0, clientX: 500 });
    fireEvent.pointerUp(window, { clientX: 500 });
    expect(onSeek).toHaveBeenCalledWith(23_500);
    expect(onSeek).toHaveBeenCalledTimes(1);
    expect(onMove).not.toHaveBeenCalled();
    expect(onResize).not.toHaveBeenCalled();
  });

  it("drags the playhead along the section, clamped to it, and seeks again on release", () => {
    const { onSeek } = renderPlayhead();
    fireEvent.pointerDown(seekBar(), { button: 0, clientX: 200 });
    fireEvent.pointerMove(window, { clientX: 1500 });
    expect(playhead().style.left).toBe("100%");
    fireEvent.pointerUp(window, { clientX: 1500 });
    expect(onSeek.mock.calls.map(([ms]) => ms)).toEqual([20_125, 30_250]);
  });

  it("goes to the section's end with End, which the clock holds just before it", async () => {
    const user = userEvent.setup();
    const { onSeek } = renderPlayhead(21_250);
    playhead().focus();
    await user.keyboard("{End}");
    expect(onSeek).toHaveBeenLastCalledWith(30_250);
  });

  it("steps with the arrow keys on the focused playhead, wrapping inside the section, and Home goes to its start", async () => {
    const user = userEvent.setup();
    const { onSeek } = renderPlayhead(29_000);
    playhead().focus();
    await user.keyboard("{ArrowRight}");
    expect(onSeek).toHaveBeenLastCalledWith(19_750);
    await user.keyboard("{ArrowLeft}");
    expect(onSeek).toHaveBeenLastCalledWith(29_000);
    await user.keyboard("{ArrowLeft}");
    expect(onSeek).toHaveBeenLastCalledWith(27_000);
    await user.keyboard("{Home}");
    expect(onSeek).toHaveBeenLastCalledWith(19_000);
  });

  it("still seeks on a saved window, since seeking stores nothing", () => {
    const { onSeek } = renderPlayhead(21_250, { locked: true });
    fireEvent.pointerDown(seekBar(), { button: 0, clientX: 500 });
    fireEvent.pointerUp(window, { clientX: 500 });
    expect(onSeek).toHaveBeenCalledWith(23_500);
  });

  it("draws no playhead without playback", () => {
    renderTimeline();
    expect(screen.queryByRole("slider", { name: "Playback position" })).toBeNull();
    expect(screen.queryByTestId("seek-bar")).toBeNull();
  });
});
