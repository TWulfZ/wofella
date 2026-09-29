import { describe, expectTypeOf, it } from "vitest";
import type { InstallDto, JobId } from "./bindings";
import { mockCommands, mockIpcError, type CommandHandlers } from "./mocks";

const INSTALL: InstallDto = { id: 1, rootPath: "/osu", osuDbVersion: 20260924, detectedAt: "2026-09-28T00:00:00.000Z" };

describe("mockCommands types", () => {
  it("accepts handlers that return each command's success type", () => {
    mockCommands({
      setupSetInstallPath: () => INSTALL,
      jobsStart: async () => Promise.resolve("01J" as JobId),
      jobsCancel: () => null,
    });
  });

  it("accepts mockIpcError in place of any success value", () => {
    mockCommands({ setupStatus: () => mockIpcError("OSU_RUNNING") });
    expectTypeOf(mockIpcError).returns.toBeNever();
  });

  it("rejects a handler returning the wrong type", () => {
    mockCommands({
      // @ts-expect-error jobsStart resolves to a JobId string, not an InstallDto
      jobsStart: () => INSTALL,
    });
    // @ts-expect-error handlers are keyed by the generated command names only
    const unknownCommand: CommandHandlers = { setupFrobnicate: () => null };
    expectTypeOf(unknownCommand).toEqualTypeOf<CommandHandlers>();
  });
});
