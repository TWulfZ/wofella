import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PatternDefDto } from "@/ipc/bindings";
import type { Answer } from "../session";
import { AnswerBar, type AnswerMode } from "./AnswerBar";
import type { HoldButtonHandle } from "./HoldButton";

const TAXONOMY: PatternDefDto[] = [
  { id: "regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "two in one column" },
  { id: "regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "three or more" },
  { id: "regular.stream.jumpstream", axis: "7k.regular.stream", key: "js", description: "jumps" },
  { id: "regular.stream.handstream", axis: "7k.regular.stream", key: "hs", description: "hands" },
];

const MINI = "regular.jack.minijack";
const LONG = "regular.jack.longjack";
const JUMP = "regular.stream.jumpstream";
const HAND = "regular.stream.handstream";

const EMPTY: Answer = { patterns: [], noPattern: false, mixed: false, unsure: false, thumb: null };

class FakeResizeObserver {
  observe(): void {
    // jsdom has no layout.
  }
  unobserve(): void {
    // See observe.
  }
  disconnect(): void {
    // See observe.
  }
}

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

interface Overrides {
  mode?: AnswerMode;
  busy?: boolean;
  canSkip?: boolean;
  onSave?: () => void;
  onSkip?: () => void;
  onRemove?: (id: string) => void;
  onChipFocus?: (id: string, how: { moveFocus: boolean }) => void;
}

function renderBar(answer: Answer, overrides: Overrides = {}) {
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  return render(
    <AnswerBar
      taxonomy={TAXONOMY}
      answer={answer}
      mode={overrides.mode ?? { kind: "edit", skipped: false, canSave: true }}
      busy={overrides.busy ?? false}
      feedback={null}
      onPick={vi.fn()}
      onRemove={overrides.onRemove ?? vi.fn()}
      onNoPattern={vi.fn()}
      onFlag={vi.fn()}
      onSave={overrides.onSave ?? vi.fn()}
      onClear={vi.fn()}
      onUndo={vi.fn()}
      onSkip={overrides.onSkip ?? vi.fn()}
      canSkip={overrides.canSkip ?? true}
      onChipFocus={overrides.onChipFocus ?? vi.fn()}
    />,
  );
}

function hold(button: HTMLElement, ms: number) {
  fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
  act(() => {
    vi.advanceTimersByTime(ms);
  });
  fireEvent.pointerUp(button, { button: 0, pointerId: 1 });
}

/** Lays the summary line out as `line` px wide, each chip `chip` px and the "+N" pill `more` px. */
function stubWidths(line: number, chip: number, more: number) {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    const kind = this.dataset["fit"];
    const width = kind === "line" ? line : kind === "chip" ? chip : kind === "more" ? more : 0;
    return { width, height: 20, top: 0, left: 0, right: width, bottom: 20, x: 0, y: 0, toJSON: () => ({}) };
  });
}

describe("AnswerBar", () => {
  it("keeps the Answer region name", () => {
    renderBar(EMPTY);
    expect(screen.getByRole("region", { name: "Answer" })).toBeInTheDocument();
  });

  it("summarises chips and active flags on one line", () => {
    renderBar({ ...EMPTY, patterns: [MINI, LONG], unsure: true, thumb: "left" });
    const summary = screen.getByTestId("answer-summary");
    expect(within(summary).getByText("mj")).toBeInTheDocument();
    expect(within(summary).getByText("lj")).toBeInTheDocument();
    expect(within(summary).getByText("Unsure")).toBeInTheDocument();
    expect(within(summary).getByText("Left thumb")).toBeInTheDocument();
    expect(within(summary).queryByTestId("answer-overflow")).toBeNull();
  });

  it("puts the flag toggles and picker in the expandable panel", () => {
    renderBar(EMPTY);
    const expanded = screen.getByTestId("answer-expanded");
    expect(within(expanded).getByRole("group", { name: /flags/i })).toBeInTheDocument();
    expect(within(expanded).getByRole("button", { name: /no pattern/i })).toBeInTheDocument();
  });
});

describe("AnswerBar hold-to-confirm actions", () => {
  it("keeps icon-only Save and Skip on the collapsed line, both described as holds", () => {
    renderBar({ ...EMPTY, patterns: [MINI] });
    const expanded = screen.getByTestId("answer-expanded");
    const save = screen.getByRole("button", { name: "Save" });
    const skip = screen.getByRole("button", { name: "Skip" });
    expect(expanded).not.toContainElement(save);
    expect(expanded).not.toContainElement(skip);
    expect(save).toHaveAccessibleDescription("Hold for 1 second to save");
    expect(skip).toHaveAccessibleDescription("Hold for 1 second to skip");
    expect(save.querySelector("svg.lucide")).not.toBeNull();
    expect(skip.querySelector("svg.lucide")).not.toBeNull();
    expect(save).not.toHaveTextContent("Save");
  });

  it("saves only after a full hold, not on a click", () => {
    vi.useFakeTimers();
    const onSave = vi.fn();
    renderBar({ ...EMPTY, patterns: [MINI] }, { onSave });
    const save = screen.getByRole("button", { name: "Save" });

    fireEvent.click(save);
    hold(save, 400);
    expect(onSave).not.toHaveBeenCalled();

    hold(save, 1000);
    expect(onSave).toHaveBeenCalledOnce();
  });

  it("skips after a full hold", () => {
    vi.useFakeTimers();
    const onSkip = vi.fn();
    renderBar(EMPTY, { onSkip });

    hold(screen.getByRole("button", { name: "Skip" }), 1000);

    expect(onSkip).toHaveBeenCalledOnce();
  });

  it("disables Skip when the window cannot be skipped and Save when no pattern is chosen", () => {
    renderBar(EMPTY, { canSkip: false, mode: { kind: "edit", skipped: false, canSave: false } });
    expect(screen.getByRole("button", { name: "Skip" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("tells why a disabled Save cannot save yet, after its hold instruction", () => {
    renderBar(EMPTY, { mode: { kind: "edit", skipped: false, canSave: false } });
    expect(screen.getByRole("button", { name: "Save" })).toHaveAccessibleDescription(
      "Hold for 1 second to save Pick a pattern or No pattern to save.",
    );
  });

  it("tells why Skip is disabled on a saved window", () => {
    renderBar({ ...EMPTY, patterns: [MINI] }, { canSkip: false, mode: { kind: "saved", canUndo: true } });
    const skip = screen.getByRole("button", { name: "Skip" });
    expect(skip).toBeDisabled();
    expect(skip).toHaveAccessibleDescription("Hold for 1 second to skip A saved window cannot be skipped; undo it first.");
  });

  it("offers Undo as a plain button on a saved window", async () => {
    const onUndo = vi.fn();
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    render(
      <AnswerBar
        taxonomy={TAXONOMY}
        answer={{ ...EMPTY, patterns: [MINI] }}
        mode={{ kind: "saved", canUndo: true }}
        busy={false}
        feedback={null}
        onPick={vi.fn()}
        onRemove={vi.fn()}
        onNoPattern={vi.fn()}
        onFlag={vi.fn()}
        onSave={vi.fn()}
        onClear={vi.fn()}
        onUndo={onUndo}
        onSkip={vi.fn()}
        canSkip={false}
        onChipFocus={vi.fn()}
      />,
    );
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Undo" }));

    expect(onUndo).toHaveBeenCalledOnce();
  });
});

describe("AnswerBar chips", () => {
  it("shows as many chips as fit and a +N pill naming the hidden ones", () => {
    // 200 px line, 60 px chips, 6 px gaps, 28 px pill: two chips and the pill fit, a third does not.
    stubWidths(200, 60, 28);
    renderBar({ ...EMPTY, patterns: [MINI, LONG, JUMP, HAND] });

    const summary = screen.getByTestId("answer-summary");
    expect(within(summary).getByText("mj")).toBeInTheDocument();
    expect(within(summary).getByText("lj")).toBeInTheDocument();
    expect(within(summary).queryByText("js")).toBeNull();
    expect(within(summary).queryByText("hs")).toBeNull();
    const more = within(summary).getByTestId("answer-overflow");
    expect(more).toHaveTextContent("+2");
    // The summary line is aria-hidden, so the hidden names are spoken from outside it.
    const spoken = screen.getByText("2 more: jumpstream, handstream");
    expect(spoken.closest("[aria-hidden]")).toBeNull();
    expect(summary).not.toContainElement(spoken);
  });

  it("counts hidden flags in the +N pill too", () => {
    stubWidths(200, 60, 28);
    renderBar({ ...EMPTY, patterns: [MINI, LONG, JUMP], mixed: true });

    const more = screen.getByTestId("answer-overflow");
    expect(more).toHaveTextContent("+2");
    expect(screen.getByText("2 more: jumpstream, Mixed").closest("[aria-hidden]")).toBeNull();
  });

  it("shows every chip and no pill when they all fit", () => {
    stubWidths(400, 60, 28);
    renderBar({ ...EMPTY, patterns: [MINI, LONG, JUMP, HAND] });

    expect(screen.queryByTestId("answer-overflow")).toBeNull();
    expect(within(screen.getByTestId("answer-summary")).getByText("hs")).toBeInTheDocument();
  });

  it("takes focus to the pattern when its chip is clicked, while × still removes it", async () => {
    const onChipFocus = vi.fn();
    const onRemove = vi.fn();
    renderBar({ ...EMPTY, patterns: [MINI, LONG] }, { onChipFocus, onRemove });
    const expanded = screen.getByTestId("answer-expanded");

    await userEvent.click(within(expanded).getByRole("button", { name: "Show longjack in the pattern panel" }));
    // A pointer click leaves focus where it was, so Enter still saves; the card is only revealed.
    expect(onChipFocus).toHaveBeenCalledExactlyOnceWith(LONG, { moveFocus: false });
    expect(onRemove).not.toHaveBeenCalled();

    await userEvent.click(within(expanded).getByRole("button", { name: "Remove minijack" }));
    expect(onRemove).toHaveBeenCalledExactlyOnceWith(MINI);
    expect(onChipFocus).toHaveBeenCalledOnce();
  });

  it("still takes a chip to its pattern on a saved window", async () => {
    const onChipFocus = vi.fn();
    renderBar({ ...EMPTY, patterns: [MINI] }, { onChipFocus, mode: { kind: "saved", canUndo: true } });

    await userEvent.click(screen.getByRole("button", { name: "Show minijack in the pattern panel" }));

    expect(onChipFocus).toHaveBeenCalledExactlyOnceWith(MINI, { moveFocus: false });
  });

  it("moves focus to the card when the chip is activated from the keyboard", async () => {
    const onChipFocus = vi.fn();
    renderBar({ ...EMPTY, patterns: [MINI] }, { onChipFocus });
    screen.getByRole("button", { name: "Show minijack in the pattern panel" }).focus();

    await userEvent.keyboard("{Enter}");

    expect(onChipFocus).toHaveBeenCalledExactlyOnceWith(MINI, { moveFocus: true });
  });

  it("holds Save and Skip for the given duration and lets the screen drive Save's hold", () => {
    vi.useFakeTimers();
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    const onSave = vi.fn();
    const saveRef = createRef<HoldButtonHandle>();
    render(
      <AnswerBar
        taxonomy={TAXONOMY}
        answer={{ ...EMPTY, patterns: [MINI] }}
        mode={{ kind: "edit", skipped: false, canSave: true }}
        busy={false}
        feedback={null}
        onPick={vi.fn()}
        onRemove={vi.fn()}
        onNoPattern={vi.fn()}
        onFlag={vi.fn()}
        onSave={onSave}
        onClear={vi.fn()}
        onUndo={vi.fn()}
        onSkip={vi.fn()}
        canSkip
        onChipFocus={vi.fn()}
        holdMs={2000}
        saveRef={saveRef}
      />,
    );
    expect(screen.getByRole("button", { name: "Skip" })).toHaveAccessibleDescription("Hold for 2 seconds to skip");

    act(() => {
      saveRef.current?.press();
    });
    act(() => {
      vi.advanceTimersByTime(1999);
    });
    expect(onSave).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(onSave).toHaveBeenCalledOnce();
  });
});
