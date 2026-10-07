// The label session: windows shown, their outcomes and the answer being built. IPC calls happen outside and only their
// outcomes are dispatched here.
import type { Anchor, LabelSelection, LabelWindow, ThumbSide } from "./types";

export interface Answer {
  /** Full taxonomy ids in pick order; never typed, so only ids the taxonomy offers can be stored. */
  patterns: readonly string[];
  /** "No clear pattern": a stored answer, exclusive with `patterns`. */
  noPattern: boolean;
  mixed: boolean;
  unsure: boolean;
  thumb: ThumbSide | null;
}

export const EMPTY_ANSWER: Answer = { patterns: [], noPattern: false, mixed: false, unsure: false, thumb: null };

export type WindowOrigin =
  | { kind: "plan"; round: number }
  | { kind: "random" }
  | { kind: "nowPlaying"; source: "osuWindow" | "lastReplay" }
  /** A map the player chose from the session list (ADR 0020). */
  | { kind: "session" };

export type EntryStatus =
  | { kind: "pending" }
  | { kind: "saved"; eventId: string; answer: Answer }
  | { kind: "skipped" };

export interface HistoryEntry {
  /** Its anchor follows timeline moves. */
  window: LabelWindow;
  origin: WindowOrigin;
  /** Set by any applied timeline move or resize, even one that ends on the offered bounds: the labeller looked elsewhere. */
  moved: boolean;
  status: EntryStatus;
}

export interface SessionCounts {
  labelled: number;
  skipped: number;
  undone: number;
}

export interface SessionState {
  /** A `u64` in decimal, as the sampler takes it. */
  seed: string;
  /** The next stratified round; only planned windows consume one. */
  round: number;
  randomRound: number;
  /** Every window shown this session, so no source shows one twice. */
  shown: Anchor[];
  /** Event ids saved this session, newest last: the undo stack. */
  saved: string[];
  history: HistoryEntry[];
  /** Index into `history`; one past its end while the next planned window is sampled. */
  cursor: number;
  answer: Answer;
  done: boolean;
  counts: SessionCounts;
}

export type FlagToggle = "mixed" | "unsure" | "thumbLeft" | "thumbRight";

export type SessionAction =
  | { type: "windowLoaded"; window: LabelWindow; origin: WindowOrigin }
  /** `cursor` is the entry the move was asked for: the reply may land after Previous or Next. */
  | { type: "windowMoved"; cursor: number; anchor: Anchor }
  | { type: "moved"; to: "previous" | "next" }
  | { type: "submitted"; eventId: string; answer: Answer }
  /** Dispatched only once the undo is stored, so a failed undo can be retried. */
  | { type: "undone"; eventId: string }
  | { type: "skipped" }
  | { type: "patternToggled"; id: string }
  /** The "+" picker only adds; removal stays on the chip and the card. */
  | { type: "patternAdded"; id: string }
  | { type: "patternRemoved"; id: string }
  | { type: "noPatternToggled" }
  | { type: "flagsToggled"; toggle: FlagToggle }
  | { type: "answerCleared" }
  | { type: "sessionDone" };

export function initialSession(seed: string): SessionState {
  return {
    seed,
    round: 0,
    randomRound: 0,
    shown: [],
    saved: [],
    history: [],
    cursor: 0,
    answer: EMPTY_ANSWER,
    done: false,
    counts: { labelled: 0, skipped: 0, undone: 0 },
  };
}

export function currentEntry(state: SessionState): HistoryEntry | null {
  return state.history[state.cursor] ?? null;
}

export function isSampling(state: SessionState): boolean {
  return !state.done && state.cursor === state.history.length;
}

function sameAnchor(a: Anchor, b: Anchor): boolean {
  return a.md5 === b.md5 && a.t0Ms === b.t0Ms && a.t1Ms === b.t1Ms;
}

function toggledThumb(current: ThumbSide | null, side: ThumbSide): ThumbSide | null {
  return current === side ? null : side;
}

function withFlag(answer: Answer, toggle: FlagToggle): Answer {
  switch (toggle) {
    case "mixed":
      return { ...answer, mixed: !answer.mixed };
    case "unsure":
      return { ...answer, unsure: !answer.unsure };
    case "thumbLeft":
      return { ...answer, thumb: toggledThumb(answer.thumb, "left") };
    case "thumbRight":
      return { ...answer, thumb: toggledThumb(answer.thumb, "right") };
  }
}

function editable(state: SessionState): boolean {
  const entry = currentEntry(state);
  return entry !== null && entry.status.kind !== "saved";
}

function moveTo(state: SessionState, cursor: number): SessionState {
  return { ...state, cursor, answer: EMPTY_ANSWER };
}

function withStatus(state: SessionState, index: number, status: EntryStatus): HistoryEntry[] {
  return state.history.map((entry, i) => (i === index ? { ...entry, status } : entry));
}

function loaded(state: SessionState, window: LabelWindow, origin: WindowOrigin): SessionState {
  const known = state.history.findIndex((entry) => sameAnchor(entry.window.anchor, window.anchor));
  if (known !== -1) {
    return moveTo(state, known);
  }
  return moveTo(
    {
      ...state,
      history: [...state.history, { window, origin, moved: false, status: { kind: "pending" } }],
      shown: [...state.shown, window.anchor],
      round: origin.kind === "plan" ? state.round + 1 : state.round,
      randomRound: origin.kind === "random" ? state.randomRound + 1 : state.randomRound,
    },
    state.history.length,
  );
}

function editAnswer(state: SessionState, edit: (answer: Answer) => Answer): SessionState {
  return editable(state) ? { ...state, answer: edit(state.answer) } : state;
}

export function sessionReducer(state: SessionState, action: SessionAction): SessionState {
  switch (action.type) {
    case "windowLoaded":
      return loaded(state, action.window, action.origin);
    case "windowMoved": {
      const entry = state.history[action.cursor];
      // A saved label is tied to the anchor it was stored with.
      if (entry === undefined || entry.status.kind === "saved") {
        return state;
      }
      const window = { ...entry.window, anchor: action.anchor };
      return { ...state, history: state.history.map((e, i) => (i === action.cursor ? { ...e, window, moved: true } : e)) };
    }
    case "moved":
      if (action.to === "previous") {
        return state.cursor === 0 ? state : moveTo(state, state.cursor - 1);
      }
      // Past the newest window sits the next sample, or the "plan finished" notice; nothing lies beyond it.
      return state.cursor >= state.history.length ? state : moveTo(state, state.cursor + 1);
    case "submitted": {
      if (!editable(state)) {
        return state;
      }
      const status: EntryStatus = { kind: "saved", eventId: action.eventId, answer: action.answer };
      return moveTo(
        {
          ...state,
          history: withStatus(state, state.cursor, status),
          saved: [...state.saved, action.eventId],
          counts: { ...state.counts, labelled: state.counts.labelled + 1 },
        },
        state.cursor + 1,
      );
    }
    case "skipped": {
      const entry = currentEntry(state);
      if (entry === null || entry.status.kind === "saved") {
        return state;
      }
      const counted = entry.status.kind === "pending" ? 1 : 0;
      return moveTo(
        {
          ...state,
          history: withStatus(state, state.cursor, { kind: "skipped" }),
          counts: { ...state.counts, skipped: state.counts.skipped + counted },
        },
        state.cursor + 1,
      );
    }
    case "undone": {
      if (undoTarget(state) !== action.eventId) {
        return state;
      }
      const index = state.history.findIndex((e) => e.status.kind === "saved" && e.status.eventId === action.eventId);
      return {
        ...state,
        history: index === -1 ? state.history : withStatus(state, index, { kind: "pending" }),
        saved: state.saved.slice(0, -1),
        counts: { ...state.counts, undone: state.counts.undone + 1 },
      };
    }
    case "patternToggled":
      return editAnswer(state, (answer) =>
        answer.patterns.includes(action.id)
          ? { ...answer, patterns: answer.patterns.filter((id) => id !== action.id) }
          : { ...answer, patterns: [...answer.patterns, action.id], noPattern: false },
      );
    case "patternAdded":
      return editAnswer(state, (answer) =>
        answer.patterns.includes(action.id)
          ? answer
          : { ...answer, patterns: [...answer.patterns, action.id], noPattern: false },
      );
    case "patternRemoved":
      return editAnswer(state, (answer) => ({ ...answer, patterns: answer.patterns.filter((id) => id !== action.id) }));
    case "noPatternToggled":
      return editAnswer(state, (answer) =>
        answer.noPattern ? { ...answer, noPattern: false } : { ...answer, noPattern: true, patterns: [] },
      );
    case "flagsToggled":
      return editAnswer(state, (answer) => withFlag(answer, action.toggle));
    case "answerCleared":
      return editAnswer(state, () => EMPTY_ANSWER);
    case "sessionDone":
      return { ...state, done: true };
  }
}

export function undoTarget(state: SessionState): string | null {
  return state.saved.at(-1) ?? null;
}

/** True on a saved window whose event is the newest saved one: older ones would undo out of order. */
export function canUndo(state: SessionState): boolean {
  const status = currentEntry(state)?.status;
  return status?.kind === "saved" && status.eventId === undoTarget(state);
}

export function canSave(state: SessionState): boolean {
  return editable(state) && (state.answer.noPattern || state.answer.patterns.length > 0);
}

export function sampleExclusion(state: SessionState): { seed: string; round: number; exclude: Anchor[] } {
  return { seed: state.seed, round: state.round, exclude: state.shown };
}

export function randomRequest(state: SessionState): { seed: string; round: number; exclude: Anchor[] } {
  return { seed: state.seed, round: state.randomRound, exclude: state.shown };
}

export function nowPlayingRequest(state: SessionState): { exclude: Anchor[] } {
  return { exclude: state.shown };
}

export interface SubmitPayload {
  anchor: Anchor;
  patterns: string[];
  noPattern: boolean;
  mixed: boolean;
  unsure: boolean;
  thumbPref: ThumbSide | null;
  selection: LabelSelection;
}

const ORIGIN_PICK: Readonly<Record<WindowOrigin["kind"], LabelSelection["pick"]>> = {
  plan: "sampled",
  random: "random",
  nowPlaying: "now_playing",
  session: "session",
};

export function selectionOf(entry: HistoryEntry): LabelSelection {
  return { pick: ORIGIN_PICK[entry.origin.kind], window: entry.moved ? "moved" : "sampled" };
}

export function submitPayload(state: SessionState): SubmitPayload | null {
  const entry = currentEntry(state);
  if (entry === null || !canSave(state)) {
    return null;
  }
  const { patterns, noPattern, mixed, unsure, thumb } = state.answer;
  return {
    anchor: entry.window.anchor,
    patterns: [...patterns],
    noPattern,
    mixed,
    unsure,
    thumbPref: thumb,
    selection: selectionOf(entry),
  };
}
