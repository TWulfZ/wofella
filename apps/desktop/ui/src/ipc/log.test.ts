import { describe, expect, it } from "vitest";
import { installErrorForwarding } from "./log";
import { mockCommands } from "./mocks";

function flush() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe("installErrorForwarding", () => {
  it("forwards window errors and unhandled rejections to plugin-log at error level", async () => {
    const calls = mockCommands({});
    const uninstall = installErrorForwarding();

    window.dispatchEvent(new ErrorEvent("error", { message: "boom", filename: "app.js", lineno: 7 }));
    const rejection = new Event("unhandledrejection") as PromiseRejectionEvent;
    Object.defineProperty(rejection, "reason", { value: new Error("nope") });
    window.dispatchEvent(rejection);
    await flush();

    const logs = calls.filter((c) => c.cmd === "plugin:log|log");
    expect(logs.map((c) => c.args["message"])).toEqual(["window.onerror: boom", "unhandledrejection: Error: nope"]);
    expect(logs.map((c) => c.args["level"])).toEqual([5, 5]);
    uninstall();
  });

  it("stops forwarding once uninstalled", async () => {
    const calls = mockCommands({});
    installErrorForwarding()();

    window.dispatchEvent(new ErrorEvent("error", { message: "late" }));
    await flush();

    expect(calls.filter((c) => c.cmd === "plugin:log|log")).toEqual([]);
  });
});
