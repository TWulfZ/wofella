import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatternDefDto } from "@/ipc/bindings";
import { type MockCall, mockCommands } from "@/ipc/mocks";
import { PatternGridPicker } from "./PatternGridPicker";
import { PATTERN_GRID_PREFS } from "./patternGridPrefs";

const TAXONOMY: PatternDefDto[] = [
  { id: "7k.regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "two notes in one column" },
  { id: "7k.regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "three or more" },
  { id: "7k.regular.speed.runningman", axis: "7k.regular.speed", key: "rm", description: "an anchor under a run" },
  { id: "7k.ln.inverse.full_inverse", axis: "7k.ln.inverse", key: "fi", description: "gaps where notes would be" },
];
const [MINIJACK, LONGJACK] = TAXONOMY as [PatternDefDto, PatternDefDto, ...PatternDefDto[]];

class NoopResizeObserver {
  observe(): void {
    // Radix measures the hover card's anchor; jsdom has no layout to report.
  }
  unobserve(): void {
    // See observe.
  }
  disconnect(): void {
    // See observe.
  }
}

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", NoopResizeObserver);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function Harness({ initial, onChoose }: { initial: string | null; onChoose: (p: PatternDefDto) => void }) {
  const [chosen, setChosen] = useState<string | null>(initial);
  return (
    <PatternGridPicker
      keymode={7}
      taxonomy={TAXONOMY}
      chosen={chosen}
      onChoose={(pattern) => {
        setChosen(pattern.id);
        onChoose(pattern);
      }}
      disabled={false}
      trigger={{ label: `Dominant pattern: ${chosen ?? "none"}`, text: chosen ?? "Choose pattern" }}
      title="Dominant pattern of Alpha Song"
      description="Choose one pattern."
    />
  );
}

function renderPicker(initial: string | null = null): { calls: MockCall[]; onChoose: ReturnType<typeof vi.fn> } {
  const calls = mockCommands({
    settingsGetHandLayout: () => "k7.313_right_thumb",
    labelPatternExamples: () => [],
  });
  const onChoose = vi.fn<(p: PatternDefDto) => void>();
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <Harness initial={initial} onChoose={onChoose} />
    </QueryClientProvider>,
  );
  return { calls, onChoose };
}

function trigger(): HTMLElement {
  return screen.getByRole("button", { name: /^Dominant pattern:/ });
}

async function open(): Promise<HTMLElement> {
  await userEvent.click(trigger());
  return screen.findByRole("dialog", { name: "Dominant pattern of Alpha Song" });
}

describe("PatternGridPicker", () => {
  it("loads the examples only once opened, drawn with the viewer's hand layout", async () => {
    const { calls } = renderPicker();
    expect(calls.filter((c) => c.cmd === "label_pattern_examples")).toHaveLength(0);

    const dialog = await open();

    expect(within(dialog).getByRole("searchbox", { name: "Search patterns" })).toHaveFocus();
    await waitFor(() => {
      expect(calls.filter((c) => c.cmd === "label_pattern_examples").map((c) => c.args)).toEqual([
        { keymode: 7, layoutId: "k7.313_right_thumb" },
      ]);
    });
    expect(within(dialog).getByRole("region", { name: "RICE" })).toBeInTheDocument();
    expect(within(dialog).getByRole("region", { name: "LN" })).toBeInTheDocument();
  });

  it("chooses the clicked card and closes, returning focus to the trigger", async () => {
    const { onChoose } = renderPicker();
    const dialog = await open();
    await userEvent.type(within(dialog).getByRole("searchbox", { name: "Search patterns" }), "rice jack");

    await userEvent.click(within(dialog).getByRole("button", { name: "lj longjack" }));

    expect(onChoose).toHaveBeenCalledExactlyOnceWith(LONGJACK);
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    expect(trigger()).toHaveAccessibleName(`Dominant pattern: ${LONGJACK.id}`);
    expect(trigger()).toHaveFocus();
  });

  it.each(["{Enter}", " "])("chooses the focused card with %s", async (key) => {
    const { onChoose } = renderPicker();
    const dialog = await open();
    await userEvent.type(within(dialog).getByRole("searchbox", { name: "Search patterns" }), "mj");
    within(dialog).getByRole("button", { name: "mj minijack" }).focus();

    await userEvent.keyboard(key);

    expect(onChoose).toHaveBeenCalledExactlyOnceWith(MINIJACK);
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });

  it("shows the current choice pressed and the others not", async () => {
    renderPicker(MINIJACK.id);
    const dialog = await open();
    await userEvent.type(within(dialog).getByRole("searchbox", { name: "Search patterns" }), "jack");

    expect(within(dialog).getByRole("button", { name: "mj minijack" })).toHaveAttribute("aria-pressed", "true");
    expect(within(dialog).getByRole("button", { name: "lj longjack" })).toHaveAttribute("aria-pressed", "false");
  });

  it("reveals the current choice on open, without a search and without rewriting the saved layout", async () => {
    renderPicker(MINIJACK.id);
    const dialog = await open();

    expect(await within(dialog).findByRole("button", { name: "mj minijack" })).toHaveAttribute("aria-pressed", "true");
    expect(within(dialog).getByRole("searchbox", { name: "Search patterns" })).toHaveFocus();
    expect(within(dialog).queryByRole("button", { name: "fi full inverse" })).not.toBeInTheDocument();
    expect(window.localStorage.getItem(PATTERN_GRID_PREFS.openAxesKey)).toBeNull();
  });

  it.each([
    ["velocidad", "rm runningman", "mj minijack"],
    ["ln inverso", "fi full inverse", "rm runningman"],
    ["notas largas", "fi full inverse", "mj minijack"],
  ])("reads the other language's axis and family words in the English UI: %s", async (query, shown, hidden) => {
    renderPicker();
    const dialog = await open();

    await userEvent.type(within(dialog).getByRole("searchbox", { name: "Search patterns" }), query);

    expect(within(dialog).getByRole("button", { name: shown })).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: hidden })).not.toBeInTheDocument();
  });

  it("closes on Escape without choosing", async () => {
    const { onChoose } = renderPicker();
    await open();

    await userEvent.keyboard("{Escape}");

    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    expect(onChoose).not.toHaveBeenCalled();
    expect(trigger()).toHaveFocus();
  });

  it("clears a typed search on the first Escape and closes on the next", async () => {
    renderPicker();
    const dialog = await open();
    const search = within(dialog).getByRole("searchbox", { name: "Search patterns" });
    await userEvent.type(search, "jack");

    await userEvent.keyboard("{Escape}");

    expect(search).toHaveValue("");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });

  it("draws a caller's own trigger content in a button it styles, still named by the label and opening the grid", async () => {
    mockCommands({ settingsGetHandLayout: () => "k7.313_right_thumb", labelPatternExamples: () => [] });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <PatternGridPicker
          keymode={7}
          taxonomy={TAXONOMY}
          chosen={null}
          onChoose={vi.fn()}
          disabled={false}
          trigger={{ label: "Dominant pattern: Choose pattern", content: <span>Choose pattern</span>, className: "tile" }}
          title="Dominant pattern of Alpha Song"
          description="Choose one pattern."
        />
      </QueryClientProvider>,
    );
    const button = trigger();
    expect(button).toHaveClass("tile");
    expect(button).toHaveTextContent("Choose pattern");
    expect(button.querySelector("svg.lucide-layout-grid")).toBeNull();

    await userEvent.click(button);

    expect(await screen.findByRole("dialog", { name: "Dominant pattern of Alpha Song" })).toBeInTheDocument();
  });

  it("cannot be opened while disabled", () => {
    mockCommands({});
    const client = new QueryClient();
    render(
      <QueryClientProvider client={client}>
        <PatternGridPicker
          keymode={7}
          taxonomy={TAXONOMY}
          chosen={null}
          onChoose={vi.fn()}
          disabled
          trigger={{ label: "Dominant pattern: none", text: "Choose pattern" }}
          title="Dominant pattern of Alpha Song"
          description="Choose one pattern."
        />
      </QueryClientProvider>,
    );
    expect(trigger()).toBeDisabled();
  });
});
