import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatternDefDto, SessionPlayDto, SessionPlaysDto } from "@/ipc/bindings";
import { type CommandHandlers, type MockCall, mockCommands, mockIpcError } from "@/ipc/mocks";
import { i18n } from "@/shared/i18n";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { SessionMapStrip } from "./SessionMapStrip";

const HOLD_MS = 40;
const A = "a".repeat(32);
const B = "b".repeat(32);

const TAXONOMY: PatternDefDto[] = [
  { id: "7k.regular.jack.minijack", axis: "7k.regular.jack", key: "mj", description: "two notes in one column" },
  { id: "7k.regular.jack.longjack", axis: "7k.regular.jack", key: "lj", description: "three or more" },
];

function play(md5: string, playId: string, label: SessionPlayDto["label"] = null): SessionPlayDto {
  return {
    playId,
    md5,
    playedAt: "2026-10-07T11:50:00.000Z",
    title: "Alpha Song",
    artist: "Artist",
    version: "Hard",
    creator: "Mapper",
    stars: 4.2,
    keymode: 7,
    setId: null,
    label,
    goldWindows: 2,
  };
}

const SAVED = { eventId: "01SESSIONA", pattern: "7k.regular.jack.minijack", at: "2026-10-07T11:55:00.000Z" };

class NoopResizeObserver {
  observe(): void {
    // jsdom does no layout.
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

function renderStrip(plays: SessionPlayDto[], extra: CommandHandlers = {}) {
  const session: SessionPlaysDto = { startedAt: "2026-10-07T10:00:00.000Z", plays };
  const calls = mockCommands({
    labelTaxonomy: () => TAXONOMY,
    sessionPlays: () => session,
    sessionLabelSubmit: () => ({ id: "01NEW" }),
    sessionLabelUndo: () => null,
    settingsGetHandLayout: () => "k7.313_right_thumb",
    labelPatternExamples: () => [],
    ...extra,
  });
  renderWithRouter(<SessionMapStrip keymode={7} md5={A} title="Alpha Song" holdMs={HOLD_MS} />, { path: "/label" });
  return calls;
}

function argsOf(calls: MockCall[], cmd: string): Record<string, unknown>[] {
  return calls.filter((c) => c.cmd === cmd).map((c) => c.args);
}

async function hold(button: HTMLElement): Promise<void> {
  fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, HOLD_MS * 2));
  });
  fireEvent.pointerUp(button, { button: 0, pointerId: 1 });
}

function strip(): Promise<HTMLElement> {
  return screen.findByRole("region", { name: "Map from this session: dominant pattern" });
}

describe("SessionMapStrip", () => {
  it("saves the map's dominant pattern against its newest play, then refreshes the session list", async () => {
    const calls = renderStrip([play(A, "aa02"), play(B, "bb01"), play(A, "aa01")]);
    const region = await strip();
    expect(await within(region).findByTestId("session-status")).toHaveTextContent("Pending");
    expect(within(region).getByRole("button", { name: "Save dominant pattern" })).toBeDisabled();
    await userEvent.click(within(region).getByRole("button", { name: "Dominant pattern of Alpha Song: Choose pattern" }));
    const picker = await screen.findByRole("dialog", { name: "Dominant pattern of Alpha Song" });
    await userEvent.type(within(picker).getByRole("searchbox", { name: "Search patterns" }), "long");
    await userEvent.click(within(picker).getByRole("button", { name: "lj longjack" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    await hold(within(region).getByRole("button", { name: "Save dominant pattern" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toEqual([
        { req: { keymode: 7, md5: A, playId: "aa02", pattern: "7k.regular.jack.longjack" } },
      ]);
    });
    await waitFor(() => {
      expect(argsOf(calls, "session_plays")).toHaveLength(2);
    });
  });

  it("saves No clear pattern as a null pattern", async () => {
    const calls = renderStrip([play(A, "aa01")]);
    const region = await strip();
    const none = await within(region).findByRole("button", { name: "No clear pattern" });
    await userEvent.click(none);
    expect(none).toHaveAttribute("aria-pressed", "true");
    await hold(within(region).getByRole("button", { name: "Save dominant pattern" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_submit")).toEqual([{ req: { keymode: 7, md5: A, playId: "aa01", pattern: null } }]);
    });
  });

  it("shows the saved answer and undoes it by its event id", async () => {
    const calls = renderStrip([play(A, "aa01", SAVED)]);
    const region = await strip();
    expect(await within(region).findByTestId("session-status")).toHaveTextContent("minijack");
    expect(within(region).getByRole("button", { name: "Dominant pattern of Alpha Song: minijack" })).toBeInTheDocument();
    await userEvent.click(within(region).getByRole("button", { name: "Undo dominant pattern" }));
    await waitFor(() => {
      expect(argsOf(calls, "session_label_undo")).toEqual([{ eventId: "01SESSIONA" }]);
    });
  });

  it("says the gold windows saved here are separate evidence that never answer it", async () => {
    renderStrip([play(A, "aa01")]);
    const region = await strip();
    expect(within(region).getByText(/gold windows you save here do not answer it/)).toBeInTheDocument();
  });

  it("reads its description as its own sentence, without the map title the controls are described by", async () => {
    renderStrip([play(A, "aa01")]);
    const region = await strip();
    expect(within(region).getByText(/gold windows you save here do not answer it/)).not.toHaveTextContent("Alpha Song");
    expect(await within(region).findByRole("button", { name: "No clear pattern" })).toHaveAccessibleDescription(
      "Alpha Song",
    );
  });

  it("words a failed save in the strip", async () => {
    renderStrip([play(A, "aa01")], { sessionLabelSubmit: () => mockIpcError("NOT_FOUND", { md5: A }) });
    const region = await strip();
    await userEvent.click(await within(region).findByRole("button", { name: "No clear pattern" }));
    await hold(within(region).getByRole("button", { name: "Save dominant pattern" }));
    expect(await within(region).findByRole("alert")).toBeInTheDocument();
  });

  it("offers no answer for a map the session list no longer has", async () => {
    renderStrip([play(B, "bb01")]);
    const region = await strip();
    expect(await within(region).findByText("This map is no longer in this session's list.")).toBeInTheDocument();
    expect(within(region).queryByRole("button", { name: "Save dominant pattern" })).toBeNull();
  });

  it("shows the session list's error with a retry", async () => {
    renderStrip([], { sessionPlays: () => mockIpcError("INTERNAL") });
    const region = await strip();
    const alert = await within(region).findByRole("alert");
    expect(within(alert).getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });

  it("speaks Spanish", async () => {
    await i18n.changeLanguage("es");
    renderStrip([play(A, "aa01")]);
    const region = await screen.findByRole("region", { name: "Mapa de esta sesión: patrón dominante" });
    expect(await within(region).findByTestId("session-status")).toHaveTextContent("Pendiente");
    expect(within(region).getByRole("button", { name: "Guardar patrón dominante" })).toBeInTheDocument();
  });
});
