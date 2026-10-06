// Port of the `wolluf label` answer grammar (apps/cli/src/cmd/label.rs parse_input), so both front ends accept the
// same lines.
import type { ThumbSide, WindowOp } from "./types";

export type Answer =
  | {
      kind: "labels";
      /** As typed; the labeling service resolves and dedups them. */
      tokens: string[];
      /** `x`: the stored "no clear pattern" answer; `tokens` is then empty. */
      noPattern: boolean;
      toggleMixed: boolean;
      toggleUnsure: boolean;
      /** Set inline for this answer, overriding the window's side. */
      thumb: ThumbSide | null;
    }
  | { kind: "thumb"; side: ThumbSide }
  | { kind: "mixed" }
  | { kind: "unsure" }
  | { kind: "skip" }
  | { kind: "undo" }
  | { kind: "reshape"; op: WindowOp }
  | { kind: "help" }
  | { kind: "leave" };

// Codes, not text: the screen words them through i18n.
export type AnswerError =
  | { code: "empty" }
  | { code: "twoThumbSides" }
  | { code: "commandInLine"; word: string }
  | { code: "noPatternWithPatterns" }
  | { code: "noPatternGiven" };

export type ParsedAnswer = { ok: true; answer: Answer } | { ok: false; error: AnswerError };

const NO_PATTERN = "x";
const MIXED = "m";
const UNSURE = "?";
const THUMB_LEFT = "tl";
const THUMB_RIGHT = "tr";

const COMMANDS: ReadonlyMap<string, Answer> = new Map<string, Answer>([
  [MIXED, { kind: "mixed" }],
  [UNSURE, { kind: "unsure" }],
  ["s", { kind: "skip" }],
  ["u", { kind: "undo" }],
  ["w+", { kind: "reshape", op: "widen" }],
  ["w-", { kind: "reshape", op: "narrow" }],
  ["n", { kind: "reshape", op: "next" }],
  ["p", { kind: "reshape", op: "prev" }],
  ["h", { kind: "help" }],
  ["q", { kind: "leave" }],
  [THUMB_LEFT, { kind: "thumb", side: "left" }],
  [THUMB_RIGHT, { kind: "thumb", side: "right" }],
]);

/** Tokens a pattern key may never be. */
export const RESERVED_TOKENS: ReadonlySet<string> = new Set([...COMMANDS.keys(), NO_PATTERN]);

export function parseAnswer(line: string): ParsedAnswer {
  const words = line
    .toLowerCase()
    .split(/[\s,]+/)
    .filter((w) => w !== "");
  const [first] = words;
  if (first === undefined) {
    return { ok: false, error: { code: "empty" } };
  }
  if (words.length === 1) {
    const command = COMMANDS.get(first);
    if (command !== undefined) {
      return { ok: true, answer: command };
    }
  }
  const tokens: string[] = [];
  let noPattern = false;
  let toggleMixed = false;
  let toggleUnsure = false;
  let thumb: ThumbSide | null = null;
  for (const word of words) {
    if (word === MIXED) {
      toggleMixed = !toggleMixed;
    } else if (word === UNSURE) {
      toggleUnsure = !toggleUnsure;
    } else if (word === NO_PATTERN) {
      noPattern = true;
    } else if (word === THUMB_LEFT || word === THUMB_RIGHT) {
      const side: ThumbSide = word === THUMB_LEFT ? "left" : "right";
      if (thumb !== null && thumb !== side) {
        return { ok: false, error: { code: "twoThumbSides" } };
      }
      thumb = side;
    } else if (COMMANDS.has(word)) {
      return { ok: false, error: { code: "commandInLine", word } };
    } else {
      tokens.push(word);
    }
  }
  if (noPattern && tokens.length > 0) {
    return { ok: false, error: { code: "noPatternWithPatterns" } };
  }
  if (!noPattern && tokens.length === 0) {
    return { ok: false, error: { code: "noPatternGiven" } };
  }
  return { ok: true, answer: { kind: "labels", tokens, noPattern, toggleMixed, toggleUnsure, thumb } };
}
