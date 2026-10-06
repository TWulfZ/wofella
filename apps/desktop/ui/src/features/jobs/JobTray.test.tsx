import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import type { JobDto } from "@/ipc/bindings";
import { startEventBridge } from "@/ipc/eventBridge";
import { emitMockEvent, mockCommands } from "@/ipc/mocks";
import { i18n } from "@/shared/i18n";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { JobTray } from "./JobTray";
import { FINISHED_CAP } from "./store";
import { jobTraySink, resetJobTray } from "./tray";

const RUNNING: JobDto = {
  id: "01JRUNNING",
  kind: "sync_plays",
  status: "running",
  started: "2026-09-28T10:00:00.000Z",
  ended: null,
  summary: null,
  error: null,
};

async function renderTray(jobs: JobDto[] = []) {
  const calls = mockCommands({ jobsList: () => jobs, jobsCancel: () => null });
  const { queryClient } = renderWithRouter(<JobTray />);
  const stop = await startEventBridge(queryClient, jobTraySink);
  return { calls, stop };
}

function trayItems(): HTMLElement[] {
  return within(screen.getByRole("list", { name: "Jobs" })).queryAllByRole("listitem");
}

beforeEach(() => {
  resetJobTray();
});

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
});
