// Test-only helpers: a typed layer over `mockIPC` so a mocked handler that drifts from the generated contract fails
// the typecheck (spec 005 "ipc/mocks.ts").
import { mockIPC } from "@tauri-apps/api/mocks";
import type { InvokeArgs } from "@tauri-apps/api/core";
import { events, type commands } from "./bindings";
import { fallbackMessageKey, type ErrorCode, type IpcError } from "./client";

export type CommandName = keyof typeof commands;

type SuccessOf<R> = R extends { status: "ok"; data: infer T } ? T : never;
export type CommandResult<K extends CommandName> = SuccessOf<Awaited<ReturnType<(typeof commands)[K]>>>;

export type CommandHandlers = {
  [K in CommandName]?: (args: Record<string, unknown>) => CommandResult<K> | Promise<CommandResult<K>>;
};

// Plugin commands are not in the generated bindings; they are keyed by their raw `plugin:<name>|<cmd>` id.
export type PluginHandlers = Partial<Record<`plugin:${string}|${string}`, (args: Record<string, unknown>) => unknown>>;

export interface MockCall {
  cmd: string;
  args: Record<string, unknown>;
}

// Every command is `<feature>_<verb>`, so the camelCase binding name maps back to the wire name mechanically.
function toWireName(name: string): string {
  return name.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}

const QUIET_PLUGIN_COMMANDS: PluginHandlers = {
  "plugin:log|log": () => null,
};

function toArgs(payload: InvokeArgs | undefined): Record<string, unknown> {
  if (payload === undefined || payload instanceof ArrayBuffer || payload instanceof Uint8Array || Array.isArray(payload)) {
    return {};
  }
  return payload;
}

/** Installs the handlers and returns the live call log (commands and plugin calls, in invocation order). */
export function mockCommands(handlers: CommandHandlers, plugins: PluginHandlers = {}): MockCall[] {
  const byWire = new Map<string, (args: Record<string, unknown>) => unknown>();
  for (const [name, handler] of Object.entries(handlers) as [string, (args: Record<string, unknown>) => unknown][]) {
    byWire.set(toWireName(name), handler);
  }
  for (const [cmd, handler] of Object.entries({ ...QUIET_PLUGIN_COMMANDS, ...plugins })) {
    if (handler !== undefined) {
      byWire.set(cmd, handler);
    }
  }
  const calls: MockCall[] = [];
  mockIPC(
    (cmd, payload) => {
      const args = toArgs(payload);
      calls.push({ cmd, args });
      const handler = byWire.get(cmd);
      if (handler === undefined) {
        // An Error (not a plain object) so typedError rethrows it and the test sees the unmocked command loudly.
        throw new Error(`unmocked command ${cmd}`);
      }
      return handler(args);
    },
    { shouldMockEvents: true },
  );
  return calls;
}

/**
 * Rejects the current mocked command with a structured IpcError. Tauri rejects with the serialized plain object, and
 * tauri-specta only wraps non-Error rejections into `{status: "error"}`, so this must throw a plain object.
 */
export function mockIpcError(
  code: ErrorCode,
  args: Record<string, string> = {},
  overrides: Partial<Omit<IpcError, "code" | "args">> = {},
): never {
  const error: IpcError = {
    code,
    messageKey: fallbackMessageKey(code),
    args,
    details: null,
    retryable: false,
    ...overrides,
  };
  // eslint-disable-next-line @typescript-eslint/only-throw-error -- see the doc comment: the wire value is not an Error.
  throw error;
}

export type EventName = keyof typeof events;
export type EventPayload<K extends EventName> = Parameters<(typeof events)[K]["emit"]>[0];

export async function emitMockEvent<K extends EventName>(name: K, payload: EventPayload<K>): Promise<void> {
  // Indexing `events` with a generic key yields a union of emitters whose parameter TS intersects; K pins it.
  const event = events[name] as { emit: (p: EventPayload<K>) => Promise<void> };
  await event.emit(payload);
}
