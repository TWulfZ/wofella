import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PLAYER_FRAME_PARAMS, PlayerFrame } from "./PlayerFrame";

afterEach(() => {
  vi.useRealTimers();
});

function renderFrame(idleMs = PLAYER_FRAME_PARAMS.idleMs) {
  return render(
    <PlayerFrame
      idleMs={idleMs}
      controls={
        <>
          <button type="button">Play</button>
          <input type="range" aria-label="Timeline" />
        </>
      }
      settings={
        <>
          <input type="range" aria-label="Audio offset" />
          <select aria-label="Scroll mode">
            <option>osu!</option>
          </select>
        </>
      }
      notices={<p role="status">Audio missing</p>}
    >
      <canvas data-testid="stage" />
    </PlayerFrame>,
  );
}

function controls(): HTMLElement {
  return screen.getByRole("group", { name: "Playback controls" });
}

function shown(): boolean {
  return controls().dataset["visible"] === "true";
}

function frame(): HTMLElement {
  return screen.getByTestId("player-frame");
}

function tab(): HTMLElement {
  return screen.getByRole("button", { name: "Playback settings" });
}

function advance(ms: number) {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

describe("PlayerFrame controls overlay", () => {
  it("lays the controls over the stage's bottom edge and the notices over its top", () => {
    renderFrame();
    expect(frame()).toContainElement(screen.getByTestId("stage"));
    expect(controls()).toContainElement(screen.getByRole("button", { name: "Play" }));
    expect(screen.getByRole("status")).toHaveTextContent("Audio missing");
  });

  it("shows the controls at first and fades them out after the idle time", () => {
    vi.useFakeTimers();
    renderFrame();
    expect(shown()).toBe(true);
    advance(PLAYER_FRAME_PARAMS.idleMs - 1);
    expect(shown()).toBe(true);
    advance(1);
    expect(shown()).toBe(false);
  });

  it("shows them again when the pointer moves over the player, and hides them when it leaves", () => {
    vi.useFakeTimers();
    renderFrame();
    advance(PLAYER_FRAME_PARAMS.idleMs);
    fireEvent.pointerMove(frame());
    expect(shown()).toBe(true);
    advance(PLAYER_FRAME_PARAMS.idleMs / 2);
    fireEvent.pointerMove(frame());
    advance(PLAYER_FRAME_PARAMS.idleMs / 2);
    expect(shown()).toBe(true);
    fireEvent.pointerLeave(frame());
    expect(shown()).toBe(false);
  });

  it("keeps them while the pointer rests on them", () => {
    vi.useFakeTimers();
    renderFrame();
    fireEvent.pointerEnter(controls());
    advance(PLAYER_FRAME_PARAMS.idleMs * 3);
    expect(shown()).toBe(true);
    fireEvent.pointerLeave(controls());
    advance(PLAYER_FRAME_PARAMS.idleMs);
    expect(shown()).toBe(false);
  });

  it("shows them when a control takes keyboard focus and never hides them while focused", () => {
    vi.useFakeTimers();
    renderFrame();
    advance(PLAYER_FRAME_PARAMS.idleMs);
    act(() => {
      screen.getByRole("button", { name: "Play" }).focus();
    });
    expect(shown()).toBe(true);
    advance(PLAYER_FRAME_PARAMS.idleMs * 3);
    expect(shown()).toBe(true);
    act(() => {
      screen.getByRole("button", { name: "Play" }).blur();
    });
    advance(PLAYER_FRAME_PARAMS.idleMs);
    expect(shown()).toBe(false);
  });

  it("keeps them through a drag that started on them, even if the pointer leaves", () => {
    vi.useFakeTimers();
    renderFrame();
    fireEvent.pointerDown(screen.getByRole("slider", { name: "Timeline" }), { button: 0 });
    fireEvent.pointerLeave(frame());
    advance(PLAYER_FRAME_PARAMS.idleMs * 2);
    expect(shown()).toBe(true);
    fireEvent.pointerUp(window);
    advance(PLAYER_FRAME_PARAMS.idleMs);
    expect(shown()).toBe(false);
  });

  it("stays reachable while faded: nothing is hidden from the keyboard or assistive technology", () => {
    vi.useFakeTimers();
    renderFrame();
    advance(PLAYER_FRAME_PARAMS.idleMs);
    expect(controls()).not.toHaveAttribute("aria-hidden");
    expect(controls()).not.toHaveAttribute("inert");
  });
});

describe("PlayerFrame settings flyout", () => {
  it("opens from the edge tab into a labelled panel, focusing its first control", async () => {
    const user = userEvent.setup();
    renderFrame();
    expect(tab()).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("dialog", { name: "Playback settings" })).toBeNull();

    await user.click(tab());

    const panel = screen.getByRole("dialog", { name: "Playback settings" });
    expect(tab()).toHaveAttribute("aria-expanded", "true");
    expect(tab()).toHaveAttribute("aria-controls", panel.id);
    expect(within(panel).getByRole("slider", { name: "Audio offset" })).toHaveFocus();
  });

  it("opens from the keyboard and traps Tab inside the panel", async () => {
    const user = userEvent.setup();
    renderFrame();
    tab().focus();
    await user.keyboard("{Enter}");
    const panel = screen.getByRole("dialog", { name: "Playback settings" });
    const close = within(panel).getByRole("button", { name: "Close playback settings" });

    await user.tab();
    expect(within(panel).getByRole("combobox", { name: "Scroll mode" })).toHaveFocus();
    await user.tab();
    expect(close).toHaveFocus();
    await user.tab();
    expect(within(panel).getByRole("slider", { name: "Audio offset" })).toHaveFocus();
    await user.tab({ shift: true });
    expect(close).toHaveFocus();
  });

  it("closes on Escape without letting the key reach the screen, and gives focus back to the tab", async () => {
    const user = userEvent.setup();
    const onWindowKey = vi.fn((e: KeyboardEvent) => e.defaultPrevented);
    window.addEventListener("keydown", onWindowKey);
    renderFrame();
    await user.click(tab());

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog", { name: "Playback settings" })).toBeNull();
    expect(tab()).toHaveFocus();
    expect(onWindowKey).toHaveLastReturnedWith(true);
    window.removeEventListener("keydown", onWindowKey);
  });

  it("closes from its close button and from the tab again", async () => {
    const user = userEvent.setup();
    renderFrame();
    await user.click(tab());
    await user.click(screen.getByRole("button", { name: "Close playback settings" }));
    expect(screen.queryByRole("dialog")).toBeNull();

    await user.click(tab());
    await user.click(tab());
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("peeks open while the pointer is on the tab or the panel, and closes when it leaves", () => {
    renderFrame();
    fireEvent.pointerEnter(screen.getByTestId("settings-flyout"));
    const panel = screen.getByRole("dialog", { name: "Playback settings" });
    expect(panel).not.toContainElement(document.activeElement as HTMLElement);
    fireEvent.pointerLeave(screen.getByTestId("settings-flyout"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("pins a peeked panel once a control in it takes focus, leaving the focus on that control", () => {
    renderFrame();
    fireEvent.pointerEnter(screen.getByTestId("settings-flyout"));
    const mode = screen.getByRole("combobox", { name: "Scroll mode" });
    act(() => {
      mode.focus();
    });
    expect(tab()).toHaveAttribute("aria-expanded", "true");
    expect(mode).toHaveFocus();
    fireEvent.pointerLeave(screen.getByTestId("settings-flyout"));
    expect(screen.getByRole("dialog", { name: "Playback settings" })).toBeInTheDocument();
  });

  it("stays open after the pointer leaves once focus is inside", async () => {
    const user = userEvent.setup();
    renderFrame();
    await user.click(tab());
    fireEvent.pointerLeave(screen.getByTestId("settings-flyout"));
    expect(screen.getByRole("dialog", { name: "Playback settings" })).toBeInTheDocument();
  });

  it("closes on a pointer press outside it", async () => {
    const user = userEvent.setup();
    renderFrame();
    await user.click(tab());
    fireEvent.pointerDown(screen.getByTestId("stage"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
