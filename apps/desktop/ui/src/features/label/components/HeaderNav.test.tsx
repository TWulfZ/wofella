import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { HeaderNav, type NavState } from "./HeaderNav";

const OPEN: NavState = { previous: null, next: null, random: null, nowPlaying: null };

function renderNav(blocked: NavState = OPEN) {
  const onAction = vi.fn();
  render(<HeaderNav blocked={blocked} onAction={onAction} />);
  return { onAction, toolbar: screen.getByRole("toolbar", { name: "Windows" }) };
}

describe("HeaderNav", () => {
  it("is a toolbar of icon buttons named for assistive technology, with no Skip", () => {
    const { toolbar } = renderNav();
    const names = within(toolbar)
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label"));
    expect(names).toEqual(["Previous", "Next", "Random", "Now playing"]);
    for (const button of within(toolbar).getAllByRole("button")) {
      expect(button.textContent).toBe("");
    }
  });

  it("runs the pressed action", async () => {
    const { onAction } = renderNav();
    await userEvent.click(screen.getByRole("button", { name: "Random" }));
    expect(onAction).toHaveBeenCalledExactlyOnceWith("random");
  });

  it("keeps a blocked button focusable, says why, and runs nothing", async () => {
    const { onAction } = renderNav({ ...OPEN, previous: "label.toolbar.firstWindow" });
    const previous = screen.getByRole("button", { name: "Previous" });
    expect(previous).toHaveAttribute("aria-disabled", "true");
    expect(previous).not.toBeDisabled();
    expect(previous).toHaveAccessibleDescription("This is the first window of the session.");
    await userEvent.click(previous);
    expect(onAction).not.toHaveBeenCalled();
  });
});
