import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import type { JobFinishedDto, JobProgressDto } from "./bindings";
import { startEventBridge, type JobEventSink } from "./eventBridge";
import { emitMockEvent, mockCommands } from "./mocks";

const PROGRESS: JobProgressDto = {
  jobId: "01J00000000000000000000000",
  kind: "sync_plays",
  stage: "ingest",
  done: 10,
  total: 40,
  etaMs: 1500,
};

const FINISHED: JobFinishedDto = { jobId: PROGRESS.jobId, status: "ok", failedItems: 3 };

function setup() {
  mockCommands({});
  const queryClient = new QueryClient();
  const sink: JobEventSink = { applyProgress: vi.fn(), applyFinished: vi.fn() };
  return { queryClient, sink };
}

describe("startEventBridge", () => {
  it("DataChanged{players} invalidates players queries and leaves setup queries alone", async () => {
    const { queryClient, sink } = setup();
    queryClient.setQueryData(["players", "aliases"], 1);
    queryClient.setQueryData(["players", "profiles", 7], 2);
    queryClient.setQueryData(["setup", "status"], 3);
    const stop = await startEventBridge(queryClient, sink);

    await emitMockEvent("dataChanged", { domains: ["players"] });

    expect(queryClient.getQueryState(["players", "aliases"])?.isInvalidated).toBe(true);
    expect(queryClient.getQueryState(["players", "profiles", 7])?.isInvalidated).toBe(true);
    expect(queryClient.getQueryState(["setup", "status"])?.isInvalidated).toBe(false);
    stop();
  });

  it("forwards progress and finished events to the tray sink", async () => {
    const { queryClient, sink } = setup();
    const stop = await startEventBridge(queryClient, sink);

    await emitMockEvent("jobProgress", PROGRESS);
    await emitMockEvent("jobFinished", FINISHED);

    expect(sink.applyProgress).toHaveBeenCalledWith(PROGRESS);
    expect(sink.applyFinished).toHaveBeenCalledWith(FINISHED);
    stop();
  });

  it("stops forwarding after unlisten", async () => {
    const { queryClient, sink } = setup();
    const stop = await startEventBridge(queryClient, sink);
    stop();

    await emitMockEvent("jobProgress", PROGRESS);

    expect(sink.applyProgress).not.toHaveBeenCalled();
  });
});
