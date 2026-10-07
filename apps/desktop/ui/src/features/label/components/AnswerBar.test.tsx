import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { PatternDefDto } from "@/ipc/bindings";
import type { Answer } from "../session";
import { AnswerBar } from "./AnswerBar";

const TAXONOMY: PatternDefDto[] = [
  { id: "regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "two in one column" },
  { id: "regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "three or more" },
];

const MINI = "regular.jack.minijack";
const LONG = "regular.jack.longjack";

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

function renderBar(answer: Answer, overrides: { onSave?: () => void } = {}) {
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  return render(
    <AnswerBar
      taxonomy={TAXONOMY}
      answer={answer}
      mode={{ kind: "edit", skipped: false, canSave: true }}
      busy={false}
      feedback={null}
      onPick={vi.fn()}
      onRemove={vi.fn()}
      onNoPattern={vi.fn()}
      onFlag={vi.fn()}
      onSave={overrides.onSave ?? vi.fn()}
      onClear={vi.fn()}
      onUndo={vi.fn()}
    />,
  );
}

describe("AnswerBar", () => {
  it("keeps the Answer region name", () => {
    renderBar(EMPTY);
    expect(screen.getByRole("region", { name: "Answer" })).toBeInTheDocument();
  });

  it("summarises chips and active flags on one truncated line", () => {
    renderBar({ ...EMPTY, patterns: [MINI, LONG], unsure: true, thumb: "left" });
    const summary = screen.getByTestId("answer-summary");
    expect(summary).toHaveClass("truncate");
    expect(within(summary).getByText("mj")).toBeInTheDocument();
    expect(within(summary).getByText("lj")).toBeInTheDocument();
    expect(within(summary).getByText("Unsure")).toBeInTheDocument();
    expect(within(summary).getByText("Left thumb")).toBeInTheDocument();
  });

  it("keeps Save on the collapsed line, outside the expandable panel", () => {
    renderBar(EMPTY);
    const expanded = screen.getByTestId("answer-expanded");
    const save = screen.getByRole("button", { name: "Save" });
    expect(expanded).not.toContainElement(save);
  });

  it("puts the flag toggles and picker in the expandable panel", () => {
    renderBar(EMPTY);
    const expanded = screen.getByTestId("answer-expanded");
    expect(within(expanded).getByRole("group", { name: /flags/i })).toBeInTheDocument();
    expect(within(expanded).getByRole("button", { name: /no pattern/i })).toBeInTheDocument();
  });

  it("saves from the collapsed line", async () => {
    const onSave = vi.fn();
    renderBar({ ...EMPTY, patterns: [MINI] }, { onSave });
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSave).toHaveBeenCalledOnce();
  });
});
