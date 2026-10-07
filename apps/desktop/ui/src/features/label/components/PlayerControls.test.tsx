import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { CentrePlayButton, PlayerControls } from "./PlayerControls";

function renderControls(rate = 1, onSkip = vi.fn()) {
  return render(
    <PlayerControls
      onSkip={onSkip}
      skipMs={2000}
      playing={false}
      loading={false}
      onToggle={vi.fn()}
      clock={null}
      windowStartMs={1000}
      range="00:01.000–00:05.000"
      duration="4.0 s"
      rate={rate}
      timeline={<input type="range" aria-label="Window position" />}
    />,
  );
}

describe("PlayerControls", () => {
  it("ends the row with the time, then back, play and forward, clear of the settings tab on the left", () => {
    renderControls(1.25);
    const play = screen.getByRole("button", { name: "Play" });
    const back = screen.getByRole("button", { name: "Back 2 s" });
    const forward = screen.getByRole("button", { name: "Forward 2 s" });
    const row = play.parentElement;
    expect([...(row?.children ?? [])].slice(-3)).toEqual([back, play, forward]);
    const time = screen.getByTestId("playback-time");
    expect(time.compareDocumentPosition(back) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByText("×1.25").compareDocumentPosition(time) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(time.closest("[data-testid='playback-readout']")).toHaveClass("ml-auto", "items-end");
  });

  it("skips back and forward by the step, like a video player", async () => {
    const user = userEvent.setup();
    const onSkip = vi.fn();
    renderControls(1, onSkip);
    await user.click(screen.getByRole("button", { name: "Back 2 s" }));
    await user.click(screen.getByRole("button", { name: "Forward 2 s" }));
    expect(onSkip.mock.calls).toEqual([[-2000], [2000]]);
  });
});

describe("CentrePlayButton", () => {
  it("plays on click, Enter and Space, under its own name so it never shadows the transport's Play", async () => {
    const user = userEvent.setup();
    const onPlay = vi.fn();
    render(<CentrePlayButton onPlay={onPlay} />);
    const button = screen.getByRole("button", { name: "Play the section" });
    expect(button).toHaveAttribute("aria-keyshortcuts", "Space");

    await user.click(button);
    button.focus();
    await user.keyboard("{Enter}");
    await user.keyboard(" ");
    expect(onPlay).toHaveBeenCalledTimes(3);
  });

  it("fades in only when motion is welcome", () => {
    render(<CentrePlayButton onPlay={vi.fn()} />);
    const cls = screen.getByRole("button", { name: "Play the section" }).getAttribute("class") ?? "";
    expect(cls).toMatch(/motion-safe:fade-in-0/);
    expect(cls).not.toMatch(/(^|\s)animate-in/);
    expect(cls).toMatch(/focus-visible:ring/);
  });
});
