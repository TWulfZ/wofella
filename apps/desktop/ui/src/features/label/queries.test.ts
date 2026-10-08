import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { startEventBridge } from "@/ipc/eventBridge";
import { emitMockEvent, mockCommands } from "@/ipc/mocks";
import { labelKeys } from "./queries";

const MD5 = "a".repeat(32);

describe("labelKeys", () => {
  it("DataChanged{library} refreshes what the index derives and leaves gold-label stats alone", async () => {
    mockCommands({});
    const queryClient = new QueryClient();
    const derived = [
      labelKeys.chartDetails(MD5),
      labelKeys.chartWindow(MD5, 0, 4000, null),
      labelKeys.patternExamples(7, null),
      labelKeys.chartTimeline(7, MD5, 120),
    ];
    for (const key of derived) {
      queryClient.setQueryData(key, 1);
    }
    queryClient.setQueryData(labelKeys.stats(), 2);
    const stop = await startEventBridge(queryClient, { applyProgress: vi.fn(), applyFinished: vi.fn() });

    await emitMockEvent("dataChanged", { domains: ["library"] });

    for (const key of derived) {
      expect(queryClient.getQueryState(key)?.isInvalidated, JSON.stringify(key)).toBe(true);
    }
    expect(queryClient.getQueryState(labelKeys.stats())?.isInvalidated).toBe(false);
    stop();
  });
});
