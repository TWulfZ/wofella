import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PLAYER_FRAME_PARAMS, PlayerFrame, type PlayerFrameProps } from "./PlayerFrame";

afterEach(() => {
  vi.useRealTimers();
});

function renderFrame(idleMs = PLAYER_FRAME_PARAMS.idleMs, extra: Partial<PlayerFrameProps> = {}) {
  return render(
    <PlayerFrame
      idleMs={idleMs}
      {...extra}
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

function hoverZone(): HTMLElement {
  return screen.getByTestId("controls-hover-zone");
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

  it("shows the controls at first and fades them out after the reveal time", () => {
    vi.useFakeTimers();
    renderFrame();
    expect(shown()).toBe(true);
    advance(PLAYER_FRAME_PARAMS.revealMs - 1);
    expect(shown()).toBe(true);
    advance(1);
    expect(shown()).toBe(false);
  });

  it("shows them again when the pointer moves over the bottom zone, and hides them when it leaves the player", () => {
    vi.useFakeTimers();
    renderFrame();
    advance(PLAYER_FRAME_PARAMS.revealMs);
    fireEvent.pointerMove(hoverZone());
    expect(shown()).toBe(true);
    advance(PLAYER_FRAME_PARAMS.idleMs / 2);
    fireEvent.pointerMove(hoverZone());
    advance(PLAYER_FRAME_PARAMS.idleMs / 2);
    expect(shown()).toBe(true);
    fireEvent.pointerLeave(frame());
    expect(shown()).toBe(false);
  });

  it("once called, keeps them while the pointer moves anywhere over the preview, leaving the bottom zone", () => {
    vi.useFakeTimers();
    renderFrame();
    advance(PLAYER_FRAME_PARAMS.revealMs);
    fireEvent.pointerMove(hoverZone());
    fireEvent.pointerLeave(hoverZone(), { relatedTarget: screen.getByTestId("stage") });
    for (let i = 0; i < 4; i++) {
      advance(PLAYER_FRAME_PARAMS.idleMs - 1);
      fireEvent.pointerMove(screen.getByTestId("stage"));
    }
    expect(shown()).toBe(true);
  });

  it("hides them after 15 s without pointer movement over the preview", () => {
    vi.useFakeTimers();
    expect(PLAYER_FRAME_PARAMS.idleMs).toBe(15_000);
    renderFrame();
    fireEvent.pointerMove(hoverZone());
    fireEvent.pointerMove(screen.getByTestId("stage"));
    advance(PLAYER_FRAME_PARAMS.idleMs - 1);
    expect(shown()).toBe(true);
    advance(1);
    expect(shown()).toBe(false);
  });

  it("ignores the pointer over the rest of the stage until the bottom zone calls them, as a video player does", () => {
    vi.useFakeTimers();
    renderFrame();
    advance(PLAYER_FRAME_PARAMS.revealMs);
    fireEvent.pointerMove(screen.getByTestId("stage"));
    fireEvent.pointerMove(frame());
    expect(shown()).toBe(false);
  });

  it("sizes the hover zone to the stage's bottom quarter, under the controls", () => {
    renderFrame();
    expect(hoverZone()).toHaveClass("bottom-0");
    expect(hoverZone()).toHaveStyle({
      height: `${String(PLAYER_FRAME_PARAMS.controlsHoverZoneShare * 100)}%`,
      minHeight: `${String(PLAYER_FRAME_PARAMS.controlsHoverZoneMinPx)}px`,
    });
    expect(frame()).toContainElement(hoverZone());
    expect(hoverZone().compareDocumentPosition(controls()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("keeps the controls off the left edge, where the settings tab takes the pointer", () => {
    renderFrame();
    expect(controls()).toHaveClass("pl-8");
  });

  it("keeps them while the pointer moves over them, and hides them once it rests there for the idle time", () => {
    vi.useFakeTimers();
    renderFrame();
    fireEvent.pointerMove(hoverZone());
    fireEvent.pointerEnter(controls());
    for (let i = 0; i < 3; i++) {
      advance(PLAYER_FRAME_PARAMS.idleMs - 1);
      fireEvent.pointerMove(controls());
    }
    expect(shown()).toBe(true);
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

describe("PlayerFrame stage clicks", () => {
  function renderClickable() {
    const onStageClick = vi.fn();
    renderFrame(undefined, { onStageClick, centre: <button type="button">Play the section</button> });
    return onStageClick;
  }

  it("toggles playback on a click anywhere on the preview, the bottom zone included", async () => {
    const user = userEvent.setup();
    const onStageClick = renderClickable();
    await user.click(screen.getByTestId("stage"));
    await user.click(hoverZone());
    expect(onStageClick).toHaveBeenCalledTimes(2);
  });

  it("leaves clicks on the controls, the settings and the centre button to them", async () => {
    const user = userEvent.setup();
    const onStageClick = renderClickable();
    await user.click(screen.getByRole("button", { name: "Play" }));
    await user.click(controls());
    await user.click(tab());
    await user.click(screen.getByRole("button", { name: "Play the section" }));
    expect(onStageClick).not.toHaveBeenCalled();
  });
});

describe("PlayerFrame stage clicks with the settings open", () => {
  it("only closes the settings on the click that dismisses them, then toggles playback again", async () => {
    const user = userEvent.setup();
    const onStageClick = vi.fn();
    renderFrame(undefined, { onStageClick });
    await user.click(tab());
    expect(screen.getByRole("dialog", { name: "Playback settings" })).toBeInTheDocument();

    await user.click(screen.getByTestId("stage"));
    expect(screen.queryByRole("dialog", { name: "Playback settings" })).not.toBeInTheDocument();
    expect(onStageClick).not.toHaveBeenCalled();

    await user.click(screen.getByTestId("stage"));
    expect(onStageClick).toHaveBeenCalledTimes(1);
  });
});

describe("PlayerFrame centre slot", () => {
  it("lays its content over the middle of the stage, leaving the rest of the stage to the pointer", () => {
    renderFrame(undefined, { centre: <button type="button">Play the section</button> });
    const layer = screen.getByTestId("player-centre");
    expect(layer).toContainElement(screen.getByRole("button", { name: "Play the section" }));
    expect(layer).toHaveClass("pointer-events-none", "items-center", "justify-center");
  });

  it("draws no layer when there is nothing to centre", () => {
    renderFrame();
    expect(screen.queryByTestId("player-centre")).toBeNull();
  });
});

type Rgb = [number, number, number];

function themeColour(css: string, token: string): Rgb {
  const hue = Number(/--hue:\s*([\d.]+)/.exec(css)?.[1]);
  const m = new RegExp(`--${token}:\\s*hsl\\(var\\(--hue\\) ([\\d.]+)% ([\\d.]+)%\\)`).exec(css);
  const s = Number(m?.[1]) / 100;
  const l = Number(m?.[2]) / 100;
  const f = (n: number): number => {
    const k = (n + hue / 30) % 12;
    return 255 * (l - s * Math.min(l, 1 - l) * Math.max(-1, Math.min(k - 3, 9 - k, 1)));
  };
  return [f(0), f(8), f(4)];
}

const over = (top: Rgb, below: Rgb, alpha: number): Rgb =>
  top.map((v, i) => alpha * v + (1 - alpha) * (below[i] ?? 0)) as Rgb;

function contrast(a: Rgb, b: Rgb): number {
  const lum = (c: Rgb): number => {
    const [r, g, bl] = c.map((v) => (v / 255 <= 0.04045 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4));
    return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (bl ?? 0);
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return ((hi ?? 0) + 0.05) / ((lo ?? 0) + 0.05);
}

describe("PlayerFrame settings flyout", () => {
  it("lets the map show through its panel", async () => {
    const user = userEvent.setup();
    renderFrame();
    await user.click(tab());
    const panel = screen.getByRole("dialog", { name: "Playback settings" });
    expect(panel.className).toMatch(/\bbg-surface-raised\/[1-8]\d\b/);
    expect(panel.className).toMatch(/\bbackdrop-blur/);
  });

  it("keeps the panel's faintest text above 4.5:1 even over a white stage", async () => {
    // The faintest text the flyout's content uses: foreground at 80% (the seed line, the close button).
    const faintestTextAlpha = 0.8;
    const css = readFileSync(resolve(import.meta.dirname, "../../../app/styles.css"), "utf8");
    const user = userEvent.setup();
    renderFrame();
    await user.click(tab());
    const tint = /\bbg-surface-raised\/(\d+)\b/.exec(screen.getByRole("dialog").className)?.[1];
    const panel = over(themeColour(css, "surface-raised"), [255, 255, 255], Number(tint) / 100);
    const text = over(themeColour(css, "foreground"), panel, faintestTextAlpha);
    expect(contrast(text, panel)).toBeGreaterThanOrEqual(4.5);
  });

  it("nudges a first-time viewer towards the tab until the panel opens", async () => {
    const user = userEvent.setup();
    const onSettingsOpen = vi.fn();
    const { rerender } = renderFrame(undefined, { settingsHint: "Settings live here", onSettingsOpen });
    expect(tab()).toHaveAttribute("data-hint", "true");
    const bubble = screen.getByText("Settings live here");
    expect(tab()).toHaveAttribute("aria-describedby", bubble.id);
    expect(screen.getByTestId("settings-hint-arrow").getAttribute("class")).toMatch(/motion-safe:animate-out/);
    expect(screen.getByTestId("settings-hint-arrow").getAttribute("class")).toMatch(/motion-safe:repeat-infinite/);
    expect(screen.getByTestId("settings-hint-arrow").getAttribute("class")).not.toMatch(/(^|\s)animate-out/);
    expect(screen.getByTestId("settings-hint-arrow")).toHaveStyle({
      animationDuration: `${String(PLAYER_FRAME_PARAMS.hintPeriodMs)}ms`,
    });

    await user.click(tab());
    expect(onSettingsOpen).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Settings live here")).toBeNull();

    rerender(
      <PlayerFrame controls={null} settings={<input aria-label="Audio offset" />} settingsHint={null}>
        <canvas data-testid="stage" />
      </PlayerFrame>,
    );
    await user.keyboard("{Escape}");
    expect(tab()).not.toHaveAttribute("data-hint");
    expect(screen.queryByTestId("settings-hint-arrow")).toBeNull();
  });

  it("lets the pointer through the flyout's full-height strip, so only the tab and the panel take it", async () => {
    const user = userEvent.setup();
    renderFrame();
    expect(screen.getByTestId("settings-flyout")).toHaveClass("pointer-events-none");
    expect(tab()).toHaveClass("pointer-events-auto");
    await user.click(tab());
    expect(screen.getByRole("dialog")).toHaveClass("pointer-events-auto");
  });

  it("neither peeks nor counts the settings as found when the pointer only crosses the flyout's edge strip", () => {
    const onSettingsOpen = vi.fn();
    renderFrame(undefined, { settingsHint: "Settings live here", onSettingsOpen });
    fireEvent.pointerEnter(screen.getByTestId("settings-flyout"));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onSettingsOpen).not.toHaveBeenCalled();
  });

  it("counts a hover peek as finding the settings only once it lasts a moment", () => {
    vi.useFakeTimers();
    const onSettingsOpen = vi.fn();
    renderFrame(undefined, { settingsHint: "Settings live here", onSettingsOpen });
    fireEvent.pointerEnter(tab());
    expect(screen.getByRole("dialog", { name: "Playback settings" })).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(PLAYER_FRAME_PARAMS.settingsFoundDwellMs - 1);
    });
    fireEvent.pointerLeave(screen.getByTestId("settings-flyout"));
    act(() => {
      vi.advanceTimersByTime(PLAYER_FRAME_PARAMS.settingsFoundDwellMs);
    });
    expect(onSettingsOpen).not.toHaveBeenCalled();

    fireEvent.pointerEnter(tab());
    act(() => {
      vi.advanceTimersByTime(PLAYER_FRAME_PARAMS.settingsFoundDwellMs);
    });
    expect(onSettingsOpen).toHaveBeenCalledTimes(1);
  });

  it("counts pinning a peeked panel by focusing a control in it as finding the settings", () => {
    const onSettingsOpen = vi.fn();
    renderFrame(undefined, { settingsHint: "Settings live here", onSettingsOpen });
    fireEvent.pointerEnter(tab());
    act(() => {
      screen.getByRole("combobox", { name: "Scroll mode" }).focus();
    });
    expect(onSettingsOpen).toHaveBeenCalledTimes(1);
  });

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
    fireEvent.pointerEnter(tab());
    const panel = screen.getByRole("dialog", { name: "Playback settings" });
    expect(panel).not.toContainElement(document.activeElement as HTMLElement);
    fireEvent.pointerLeave(screen.getByTestId("settings-flyout"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("pins a peeked panel once a control in it takes focus, leaving the focus on that control", () => {
    renderFrame();
    fireEvent.pointerEnter(tab());
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
