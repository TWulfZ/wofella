import { describe, expect, expectTypeOf, it } from "vitest";
import { commands } from "./bindings";
import { call, IpcFailure, isIpcError, toIpcError, type IpcError } from "./client";
import { mockCommands, mockIpcError } from "./mocks";

const STATUS = {
  install: null,
  identityReady: false,
  dataDir: "/data",
  logsDir: "/data/logs",
  appVersion: "0.1.0",
  lastSync: null,
};

async function failureOf(p: Promise<unknown>): Promise<IpcFailure> {
  try {
    await p;
  } catch (e) {
    if (e instanceof IpcFailure) {
      return e;
    }
    throw e;
  }
  throw new Error("expected the call to fail");
}

describe("call", () => {
  it("unwraps an ok result to its value", async () => {
    mockCommands({ setupStatus: () => STATUS });
    await expect(call(commands.setupStatus())).resolves.toEqual(STATUS);
  });

  it("throws IpcFailure narrowed by code for a structured error", async () => {
    mockCommands({ jobsStart: () => mockIpcError("OSU_RUNNING", { pid: "42" }, { retryable: true }) });
    const failure = await failureOf(call(commands.jobsStart({ kind: "sync_plays", installId: 1 })));
    expect(failure.error.code).toBe("OSU_RUNNING");
    expect(failure.error.messageKey).toBe("error.code.OSU_RUNNING");
    expect(failure.error.args).toEqual({ pid: "42" });
    expect(failure.error.retryable).toBe(true);
    if (isIpcError(failure.error, "OSU_RUNNING")) {
      expectTypeOf(failure.error.code).toEqualTypeOf<"OSU_RUNNING">();
    } else {
      throw new Error("isIpcError should narrow OSU_RUNNING");
    }
  });

  it("turns a non-structured string rejection into INTERNAL with the raw value in details", async () => {
    // Tauri rejects an unknown command with a plain string, which typedError hands back as `status: "error"`.
    const failure = await failureOf(call(Promise.resolve({ status: "error" as const, error: "command foo not found" as never })));
    expect(failure.error).toEqual({
      code: "INTERNAL",
      messageKey: "error.code.INTERNAL",
      args: {},
      details: "command foo not found",
      retryable: false,
    });
  });

  it("turns a thrown Error (typedError rethrows those) into INTERNAL with the message in details", async () => {
    mockCommands({ setupStatus: () => Promise.reject(new TypeError("invalid args `path`")) });
    const failure = await failureOf(call(commands.setupStatus()));
    expect(failure.error.code).toBe("INTERNAL");
    expect(failure.error.details).toBe("TypeError: invalid args `path`");
  });
});

describe("isIpcError", () => {
  const valid: IpcError = {
    code: "NOT_FOUND",
    messageKey: "error.code.NOT_FOUND",
    args: {},
    details: null,
    retryable: false,
  };

  it("accepts a structured error and checks the code when given", () => {
    expect(isIpcError(valid)).toBe(true);
    expect(isIpcError(valid, "NOT_FOUND")).toBe(true);
    expect(isIpcError(valid, "CONFLICT")).toBe(false);
  });

  it("rejects unknown codes and malformed values", () => {
    expect(isIpcError({ ...valid, code: "NOPE" })).toBe(false);
    expect(isIpcError({ ...valid, args: { n: 1 } })).toBe(false);
    expect(isIpcError({ code: "NOT_FOUND" })).toBe(false);
    expect(isIpcError("NOT_FOUND")).toBe(false);
    expect(isIpcError(null)).toBe(false);
  });
});

describe("toIpcError", () => {
  it("passes structured errors through and unwraps IpcFailure", () => {
    const error = toIpcError({
      code: "CONFLICT",
      messageKey: "players.error.self_overlap",
      args: { profileIds: "2,3" },
      details: null,
      retryable: false,
    });
    expect(error.code).toBe("CONFLICT");
    expect(toIpcError(new IpcFailure(error))).toBe(error);
  });

  it("serializes other values into details", () => {
    expect(toIpcError({ weird: 1 }).details).toBe('{"weird":1}');
    expect(toIpcError(undefined).details).toBe("undefined");
  });
});
