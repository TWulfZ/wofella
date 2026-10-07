import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { CentrePlayButton, PlayerControls } from "./PlayerControls";

function renderControls(rate = 1) {
  return render(
    <PlayerControls
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
  it("ends the row with the time and then the play button, clear of the settings tab on the left", () => {
    renderControls(1.25);
    const play = screen.getByRole("button", { name: "Play" });
    const row = play.parentElement;
    expect(row?.lastElementChild).toBe(play);
    const time = screen.getByTestId("playback-time");
    expect(time.compareDocumentPosition(play) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByText("×1.25").compareDocumentPosition(time) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(time.closest("[data-testid='playback-readout']")).toHaveClass("ml-auto", "items-end");
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
