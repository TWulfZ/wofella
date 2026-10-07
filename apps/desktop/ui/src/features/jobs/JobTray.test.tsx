import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { JobDto } from "@/ipc/bindings";
import { startEventBridge } from "@/ipc/eventBridge";
import { emitMockEvent, mockCommands } from "@/ipc/mocks";
import { i18n } from "@/shared/i18n";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { JobTray } from "./JobTray";
import { FINISHED_CAP } from "./store";
import { jobTraySink, resetJobTray } from "./tray";
import { JOB_TRAY_PARAMS, JOB_TRAY_PREFS, type JobTrayParams } from "./trayParams";

const RUNNING: JobDto = {
  id: "01JRUNNING",
  kind: "sync_plays",
  status: "running",
  started: "2026-09-28T10:00:00.000Z",
  ended: null,
  summary: null,
  error: null,
};

// Short enough for real timers; the shipped delay is exercised by its param, not by waiting for it.
const FAST: JobTrayParams = { ...JOB_TRAY_PARAMS, autoCollapseMs: 20 };

/** Most cases read the panel, which only a viewer's own expansion opens for a background job. */
async function renderTray(jobs: JobDto[] = [], params: JobTrayParams = JOB_TRAY_PARAMS, expanded = true) {
  if (expanded) {
    localStorage.setItem(JOB_TRAY_PREFS.expandedKey, "true");
    resetJobTray();
  }
  const calls = mockCommands({ jobsList: () => jobs, jobsCancel: () => null });
  const { queryClient } = renderWithRouter(<JobTray params={params} />);
  const stop = await startEventBridge(queryClient, jobTraySink);
  return { calls, stop };
}

function trayItems(): HTMLElement[] {
  return within(screen.getByRole("list", { name: "Jobs" })).queryAllByRole("listitem");
}

beforeEach(() => {
  resetJobTray();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

describe("JobTray", () => {
  it("hydrates running jobs from jobs_list", async () => {
    await renderTray([RUNNING]);
    const item = await screen.findByRole("listitem");
    expect(item).toHaveTextContent("Sync plays");
    expect(item).toHaveTextContent("Running");
  });

  it("job-progress events update done/total and the ETA", async () => {
    await renderTray([RUNNING]);
    await screen.findByRole("listitem");

    await emitMockEvent("jobProgress", {
      jobId: RUNNING.id,
      kind: "sync_plays",
      stage: "ingest",
      done: 10,
      total: 40,
      etaMs: 125_000,
    });

    const item = await screen.findByRole("listitem");
    await waitFor(() => {
      expect(item).toHaveTextContent("10 / 40");
    });
    expect(item).toHaveTextContent("Reading scores");
    expect(item).toHaveTextContent("2:05 left");
    expect(within(item).getByRole("progressbar")).toBeInTheDocument();
  });

  it("a queued job shows no progress bar until it runs", async () => {
    await renderTray([{ ...RUNNING, status: "queued" }]);
    const item = await screen.findByRole("listitem");
    expect(item).toHaveTextContent("Queued");
    expect(within(item).queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("Cancel calls jobs_cancel with the job id", async () => {
    const { calls } = await renderTray([RUNNING]);
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(calls.find((c) => c.cmd === "jobs_cancel")?.args).toEqual({ id: RUNNING.id });
    });
  });

  it("job-finished with failed items shows the count in en and es", async () => {
    await renderTray([RUNNING]);
    await screen.findByRole("listitem");

    await emitMockEvent("jobFinished", { jobId: RUNNING.id, status: "ok", failedItems: 37 });

    const item = await screen.findByRole("listitem");
    await waitFor(() => {
      expect(item).toHaveTextContent("37 items failed");
    });
    expect(item).toHaveTextContent("Finished");
    expect(within(item).queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();

    await i18n.changeLanguage("es");
    await waitFor(() => {
      expect(screen.getByRole("listitem")).toHaveTextContent("37 elementos fallaron");
    });
  });

  it(`the ${FINISHED_CAP + 1}st finished job evicts the oldest`, async () => {
    await renderTray([RUNNING]);
    await screen.findByRole("listitem");
    await emitMockEvent("jobFinished", { jobId: RUNNING.id, status: "ok", failedItems: 1 });
    for (let i = 0; i < FINISHED_CAP; i += 1) {
      await emitMockEvent("jobFinished", { jobId: `01JNEXT${i}`, status: "cancelled", failedItems: 0 });
    }

    await waitFor(() => {
      expect(trayItems()).toHaveLength(FINISHED_CAP);
    });
    expect(screen.queryByText("1 item failed")).not.toBeInTheDocument();
  });

  it("Dismiss removes a finished job", async () => {
    await renderTray([RUNNING]);
    await screen.findByRole("listitem");
    await emitMockEvent("jobFinished", { jobId: RUNNING.id, status: "failed", failedItems: 0 });

    await userEvent.click(await screen.findByRole("button", { name: "Dismiss" }));

    expect(trayItems()).toHaveLength(0);
  });

  it("collapses and reopens", async () => {
    await renderTray([RUNNING]);
    await screen.findByRole("listitem");

    await userEvent.click(screen.getByRole("button", { name: "Hide jobs" }));
    expect(screen.queryByRole("listitem")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /Jobs/ }));
    expect(await screen.findByRole("listitem")).toBeInTheDocument();
  });

  it("rests as a collapsed edge tab", async () => {
    await renderTray([], JOB_TRAY_PARAMS, false);
    const tab = await screen.findByRole("button", { name: "Jobs" });
    expect(tab).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("region", { name: "Jobs" })).not.toBeInTheDocument();
  });

  it("Enter on the tab expands and moves focus into the panel", async () => {
    await renderTray([], JOB_TRAY_PARAMS, false);
    (await screen.findByRole("button", { name: "Jobs" })).focus();
    await userEvent.keyboard("{Enter}");

    expect(screen.getByRole("region", { name: "Jobs" })).toBeInTheDocument();
    const hide = screen.getByRole("button", { name: "Hide jobs" });
    expect(hide).toHaveAttribute("aria-expanded", "true");
    expect(hide).toHaveFocus();
    expect(localStorage.getItem(JOB_TRAY_PREFS.expandedKey)).toBe("true");
  });

  it("Esc collapses back to the focused tab and remembers the choice", async () => {
    await renderTray([RUNNING]);
    await screen.findByRole("listitem");
    screen.getByRole("button", { name: "Hide jobs" }).focus();

    await userEvent.keyboard("{Escape}");

    expect(screen.queryByRole("region", { name: "Jobs" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Jobs/ })).toHaveFocus();
    expect(localStorage.getItem(JOB_TRAY_PREFS.expandedKey)).toBe("false");
  });

  it("a remembered collapse keeps a new job behind the tab, which still shows it running", async () => {
    localStorage.setItem(JOB_TRAY_PREFS.expandedKey, "false");
    resetJobTray();
    await renderTray([RUNNING], JOB_TRAY_PARAMS, false);

    expect(await screen.findByRole("button", { name: /1 running/ })).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("listitem")).not.toBeInTheDocument();
  });

  it("with no remembered choice, a running job stays behind the tab, which shows it running", async () => {
    await renderTray([RUNNING], JOB_TRAY_PARAMS, false);
    expect(await screen.findByRole("button", { name: /1 running/ })).toHaveAttribute("aria-expanded", "false");
    await emitMockEvent("jobProgress", { jobId: "01JWATCHER", kind: "sync_plays", stage: "ingest", done: 1, total: 2, etaMs: null });
    expect(await screen.findByRole("button", { name: /2 running/ })).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("region", { name: "Jobs" })).not.toBeInTheDocument();
  });

  it("keeps a finished job with failed items flagged on the tab after the auto-collapse", async () => {
    await renderTray([RUNNING], FAST);
    await screen.findByRole("listitem");
    await emitMockEvent("jobFinished", { jobId: RUNNING.id, status: "ok", failedItems: 37 });
    expect(await screen.findByRole("button", { name: /1 failed/ })).toHaveAttribute("aria-expanded", "false");
  });

  it("sizes the tab to the gutter that full-width screens keep free for it", async () => {
    await renderTray([], JOB_TRAY_PARAMS, false);
    const tab = await screen.findByRole("button", { name: "Jobs" });
    expect(tab).toHaveClass("w-(--job-tray-tab-w)");
  });

  it("a remembered expansion opens the tray from the start", async () => {
    localStorage.setItem(JOB_TRAY_PREFS.expandedKey, "true");
    resetJobTray();
    await renderTray();
    expect(await screen.findByRole("region", { name: "Jobs" })).toHaveTextContent("No jobs yet.");
  });

  it("auto-collapses once every job finished, keeping a failure indicator on the tab", async () => {
    await renderTray([RUNNING], FAST);
    await screen.findByRole("listitem");

    await emitMockEvent("jobFinished", { jobId: RUNNING.id, status: "failed", failedItems: 0 });

    const tab = await screen.findByRole("button", { name: /1 failed/ });
    expect(tab).toHaveAttribute("aria-expanded", "false");
    // The tray hid itself; that is not the user choosing to keep it collapsed.
    expect(localStorage.getItem(JOB_TRAY_PREFS.expandedKey)).toBe("true");
  });

  it("holds the auto-collapse while focus is inside the panel", async () => {
    await renderTray([RUNNING], FAST);
    await screen.findByRole("listitem");
    const hide = screen.getByRole("button", { name: "Hide jobs" });
    hide.focus();

    await emitMockEvent("jobFinished", { jobId: RUNNING.id, status: "ok", failedItems: 0 });
    await wait(FAST.autoCollapseMs * 4);
    expect(screen.getByRole("region", { name: "Jobs" })).toBeInTheDocument();

    hide.blur();
    expect(await screen.findByRole("button", { name: "Jobs" })).toHaveAttribute("aria-expanded", "false");
  });

  it("an expansion the user asks for after the jobs finished does not auto-collapse", async () => {
    await renderTray([RUNNING], FAST);
    await screen.findByRole("listitem");
    await emitMockEvent("jobFinished", { jobId: RUNNING.id, status: "ok", failedItems: 0 });
    await userEvent.click(await screen.findByRole("button", { name: "Jobs" }));

    await wait(FAST.autoCollapseMs * 4);
    expect(screen.getByRole("region", { name: "Jobs" })).toBeInTheDocument();
  });

  it("keeps working when storage throws", async () => {
    const broken = () => {
      throw new Error("blocked");
    };
    vi.stubGlobal("localStorage", { getItem: broken, setItem: broken, removeItem: broken, clear: broken });
    resetJobTray();
    await renderTray([], JOB_TRAY_PARAMS, false);

    await userEvent.click(await screen.findByRole("button", { name: "Jobs" }));
    expect(screen.getByRole("region", { name: "Jobs" })).toBeInTheDocument();
  });
});
