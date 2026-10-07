import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createRef } from "react";
import { HoldButton, type HoldButtonHandle } from "./HoldButton";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

function renderHold(props: Partial<Parameters<typeof HoldButton>[0]> = {}) {
  const onConfirm = vi.fn();
  const label = props.label ?? "Save";
  render(<HoldButton label={label} icon={<svg />} onConfirm={onConfirm} {...props} />);
  return { onConfirm, button: screen.getByRole("button", { name: label }) };
}

function advance(ms: number) {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

function ringProgress(): number {
  return Number(screen.getByTestId("hold-ring").getAttribute("data-progress"));
}

describe("HoldButton", () => {
  it("is named by its label and described by the hold instruction", () => {
    const { button } = renderHold();
    expect(button).toHaveAccessibleDescription("Hold for 0.5 seconds to save");
  });

  it("confirms once a pointer hold reaches 500 ms, filling the ring on the way", () => {
    const { onConfirm, button } = renderHold();

    fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
    advance(250);
    expect(onConfirm).not.toHaveBeenCalled();
    expect(ringProgress()).toBeGreaterThan(0.3);
    expect(ringProgress()).toBeLessThan(0.7);

    advance(249);
    expect(onConfirm).not.toHaveBeenCalled();
    advance(1);
    expect(onConfirm).toHaveBeenCalledOnce();

    fireEvent.pointerUp(button, { button: 0, pointerId: 1 });
    expect(ringProgress()).toBe(0);
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("cancels when the pointer is released early, and resets the ring", () => {
    const { onConfirm, button } = renderHold();

    fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
    advance(300);
    fireEvent.pointerUp(button, { button: 0, pointerId: 1 });
    advance(1000);

    expect(onConfirm).not.toHaveBeenCalled();
    expect(ringProgress()).toBe(0);
  });

  it("cancels when the pointer leaves the button", () => {
    const { onConfirm, button } = renderHold();

    fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
    advance(200);
    fireEvent.pointerLeave(button, { pointerId: 1 });
    advance(1000);

    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("ignores a plain click and secondary buttons", () => {
    const { onConfirm, button } = renderHold();

    fireEvent.click(button);
    fireEvent.pointerDown(button, { button: 2, pointerId: 1 });
    advance(1500);

    expect(onConfirm).not.toHaveBeenCalled();
  });

  it.each(["Enter", " "])("confirms while %j is held on the focused button, ignoring key repeats", (key) => {
    const { onConfirm, button } = renderHold();
    button.focus();

    fireEvent.keyDown(button, { key });
    advance(250);
    fireEvent.keyDown(button, { key, repeat: true });
    advance(250);

    expect(onConfirm).toHaveBeenCalledOnce();
    fireEvent.keyUp(button, { key });
  });

  it("cancels a keyboard hold released early or when focus leaves", () => {
    const { onConfirm, button } = renderHold();
    button.focus();

    fireEvent.keyDown(button, { key: "Enter" });
    advance(350);
    fireEvent.keyUp(button, { key: "Enter" });
    fireEvent.keyDown(button, { key: " " });
    advance(350);
    fireEvent.blur(button);
    advance(1000);

    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("words a one-second hold in the singular", () => {
    const { button } = renderHold({ holdMs: 1000 });
    expect(button).toHaveAccessibleDescription("Hold for 1 second to save");
  });

  it("honours a custom hold duration", () => {
    const { onConfirm, button } = renderHold({ holdMs: 2000 });
    expect(button).toHaveAccessibleDescription("Hold for 2 seconds to save");

    fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
    advance(1999);
    expect(onConfirm).not.toHaveBeenCalled();
    advance(1);
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("does nothing while disabled", () => {
    const { onConfirm, button } = renderHold({ disabled: true });
    expect(button).toBeDisabled();

    fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
    fireEvent.keyDown(button, { key: "Enter" });
    advance(1500);

    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("drops a hold in progress when it becomes disabled", () => {
    const onConfirm = vi.fn();
    const { rerender } = render(<HoldButton label="Skip" icon={<svg />} onConfirm={onConfirm} />);
    fireEvent.pointerDown(screen.getByRole("button", { name: "Skip" }), { button: 0, pointerId: 1 });
    advance(250);

    rerender(<HoldButton label="Skip" icon={<svg />} onConfirm={onConfirm} disabled />);
    advance(1000);

    expect(onConfirm).not.toHaveBeenCalled();
    expect(ringProgress()).toBe(0);
  });

  it("runs a hold driven from outside through its handle, filling the same ring", () => {
    const ref = createRef<HoldButtonHandle>();
    const onConfirm = vi.fn();
    render(<HoldButton ref={ref} label="Save" icon={<svg />} onConfirm={onConfirm} />);

    act(() => {
      ref.current?.press();
    });
    advance(250);
    expect(ringProgress()).toBeGreaterThan(0.3);
    act(() => {
      ref.current?.release();
    });
    advance(1000);
    expect(onConfirm).not.toHaveBeenCalled();
    expect(ringProgress()).toBe(0);

    act(() => {
      ref.current?.press();
    });
    advance(1000);
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("cancels a pointer hold when the window loses focus", () => {
    const { onConfirm, button } = renderHold();

    fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
    advance(200);
    fireEvent.blur(window);
    advance(1000);

    expect(onConfirm).not.toHaveBeenCalled();
    expect(ringProgress()).toBe(0);
  });

  it("cancels a hold driven from outside when the page is hidden, then accepts the next press", () => {
    const ref = createRef<HoldButtonHandle>();
    const onConfirm = vi.fn();
    render(<HoldButton ref={ref} label="Save" icon={<svg />} onConfirm={onConfirm} />);
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");

    act(() => {
      ref.current?.press();
    });
    advance(200);
    fireEvent(document, new Event("visibilitychange"));
    advance(1000);
    expect(onConfirm).not.toHaveBeenCalled();

    visibility.mockReturnValue("visible");
    // No release came for the cancelled hold: a new press must still start one.
    act(() => {
      ref.current?.press();
    });
    advance(1000);
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("lets a new press start after a confirmed hold whose release never came", () => {
    const ref = createRef<HoldButtonHandle>();
    const onConfirm = vi.fn();
    render(<HoldButton ref={ref} label="Save" icon={<svg />} onConfirm={onConfirm} />);

    act(() => {
      ref.current?.press();
    });
    advance(1000);
    act(() => {
      ref.current?.press();
    });
    advance(1000);

    expect(onConfirm).toHaveBeenCalledTimes(2);
  });

  it("adds an extra description after the hold instruction", () => {
    render(
      <>
        <p id="why">Pick a pattern first.</p>
        <HoldButton label="Save" icon={<svg />} onConfirm={vi.fn()} disabled describedBy="why" />
      </>,
    );
    expect(screen.getByRole("button", { name: "Save" })).toHaveAccessibleDescription(
      "Hold for 0.5 seconds to save Pick a pattern first.",
    );
  });

  it("ignores a press through the handle while disabled", () => {
    const ref = createRef<HoldButtonHandle>();
    const onConfirm = vi.fn();
    render(<HoldButton ref={ref} label="Save" icon={<svg />} onConfirm={onConfirm} disabled />);
    act(() => {
      ref.current?.press();
    });
    advance(1500);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("speaks the description in Spanish", async () => {
    const { i18n } = await import("@/shared/i18n");
    await act(async () => {
      await i18n.changeLanguage("es");
    });
    const { button } = renderHold({ label: "Guardar" });
    expect(button).toHaveAccessibleDescription("Mantén pulsado 0,5 segundos para guardar");
  });
});
