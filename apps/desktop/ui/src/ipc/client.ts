import type { ErrorCodeDto, IpcError as RawIpcError } from "./bindings";

export type ErrorCode = ErrorCodeDto;

// A true discriminated union derived from the generated type, so `switch (error.code)` narrows (spec 005 Design).
export type IpcError = { [C in ErrorCode]: Omit<RawIpcError, "code"> & { code: C } }[ErrorCode];

export type Result<T, E> = { status: "ok"; data: T } | { status: "error"; error: E };

// Kept in step with ErrorCodeDto by the `satisfies` below: a new code breaks the build until it is listed.
const ERROR_CODES_RECORD = {
  OSU_DIR_NOT_FOUND: true,
  UNSUPPORTED_FORMAT: true,
  PARSE_FAILED: true,
  OSU_RUNNING: true,
  CONSENT_REQUIRED: true,
  SIGNATURE_INVALID: true,
  NOT_FOUND: true,
  INVALID_INPUT: true,
  CONFLICT: true,
  CANCELLED: true,
  INTERNAL: true,
} satisfies Record<ErrorCode, true>;

export const ERROR_CODES = Object.keys(ERROR_CODES_RECORD) as readonly ErrorCode[];

export function fallbackMessageKey(code: ErrorCode): string {
  return `error.code.${code}`;
}

export class IpcFailure extends Error {
  readonly error: IpcError;

  constructor(error: IpcError) {
    super(`${error.code}: ${error.messageKey}`);
    this.name = "IpcFailure";
    this.error = error;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === "string" && Object.hasOwn(ERROR_CODES_RECORD, value);
}

function isStringMap(value: unknown): value is Record<string, string> {
  return isRecord(value) && Object.values(value).every((v) => typeof v === "string");
}

export function isIpcError<C extends ErrorCode>(value: unknown, code?: C): value is Extract<IpcError, { code: C }> {
  return (
    isRecord(value) &&
    isErrorCode(value["code"]) &&
    (code === undefined || value["code"] === code) &&
    typeof value["messageKey"] === "string" &&
    isStringMap(value["args"]) &&
    (value["details"] === null || typeof value["details"] === "string") &&
    typeof value["retryable"] === "boolean"
  );
}

export function describeRaw(value: unknown): string {
  if (value instanceof Error) {
    return `${value.name}: ${value.message}`;
  }
  if (typeof value === "string") {
    return value;
  }
  try {
    // lib.d.ts types the result as string, but undefined, functions and symbols serialize to undefined.
    const json = JSON.stringify(value) as string | undefined;
    return json ?? String(value);
  } catch {
    return String(value);
  }
}

// Unknown commands, argument deserialization failures and plugin errors are not structured IpcErrors; the UI still
// needs one shape to render, so they become INTERNAL with the raw value kept for the logs.
export function toIpcError(value: unknown): IpcError {
  if (value instanceof IpcFailure) {
    return value.error;
  }
  if (isIpcError(value)) {
    return value;
  }
  return {
    code: "INTERNAL",
    messageKey: fallbackMessageKey("INTERNAL"),
    args: {},
    details: describeRaw(value),
    retryable: false,
  };
}

export async function call<T>(pending: Promise<Result<T, RawIpcError>>): Promise<T> {
  let result: Result<T, RawIpcError>;
  try {
    result = await pending;
  } catch (e) {
    // tauri-specta's typedError rethrows Error instances instead of wrapping them.
    throw new IpcFailure(toIpcError(e));
  }
  if (result.status === "ok") {
    return result.data;
  }
  throw new IpcFailure(toIpcError(result.error));
}
