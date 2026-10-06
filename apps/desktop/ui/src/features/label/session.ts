// Mirrors the `wolluf label` REPL session (apps/cli/src/cmd/label.rs session()); IPC calls happen outside and only
// their outcomes are dispatched here.
import type { Anchor, LabelWindow, ThumbSide } from "./types";

export interface SessionFlags {
  mixed: boolean;
  unsure: boolean;
  thumb: ThumbSide | null;
}

export interface SessionCounts {
  labelled: number;
  skipped: number;
  undone: number;
}

export interface SessionState {
  /** A `u64` in decimal, as the sampler takes it. */
  seed: string;
  round: number;
  /** Every window sampled this session, so the sampler never shows one twice (skips are not stored). */
  shown: Anchor[];
  /** Event ids saved this session, newest last: the undo stack. */
  saved: string[];
  flags: SessionFlags;
  /** The current window, its anchor following reshapes. */
  window: LabelWindow | null;
  done: boolean;
  counts: SessionCounts;
}

export type FlagToggle = "mixed" | "unsure" | "thumbLeft" | "thumbRight";

export type SessionAction =
  | { type: "windowLoaded"; window: LabelWindow }
  | { type: "windowReshaped"; anchor: Anchor }
  | { type: "submitted"; eventId: string }
  /** Dispatched only once the undo is stored, so a failed undo can be retried. */
  | { type: "undone"; eventId: string }
  | { type: "skipped" }
  | { type: "flagsToggled"; toggle: FlagToggle }
  | { type: "sessionDone" };

const NEUTRAL_FLAGS: SessionFlags = { mixed: false, unsure: false, thumb: null };

export function initialSession(seed: string): SessionState {
  return {
    seed,
    round: 0,
    shown: [],
    saved: [],
    flags: NEUTRAL_FLAGS,
    window: null,
    done: false,
    counts: { labelled: 0, skipped: 0, undone: 0 },
  };
}

function toggledThumb(current: ThumbSide | null, side: ThumbSide): ThumbSide | null {
  return current === side ? null : side;
}

function toggled(flags: SessionFlags, toggle: FlagToggle): SessionFlags {
  switch (toggle) {
    case "mixed":
      return { ...flags, mixed: !flags.mixed };
    case "unsure":
      return { ...flags, unsure: !flags.unsure };
    case "thumbLeft":
      return { ...flags, thumb: toggledThumb(flags.thumb, "left") };
    case "thumbRight":
      return { ...flags, thumb: toggledThumb(flags.thumb, "right") };
  }
}

function nextRound(state: SessionState, counts: SessionCounts): SessionState {
  return { ...state, round: state.round + 1, window: null, counts };
}

export function sessionReducer(state: SessionState, action: SessionAction): SessionState {
  switch (action.type) {
    case "windowLoaded":
      return { ...state, window: action.window, shown: [...state.shown, action.window.anchor], flags: NEUTRAL_FLAGS };
    case "windowReshaped":
      return state.window === null ? state : { ...state, window: { ...state.window, anchor: action.anchor } };
    case "submitted":
      return nextRound(
        { ...state, saved: [...state.saved, action.eventId] },
        { ...state.counts, labelled: state.counts.labelled + 1 },
      );
    case "skipped":
      return nextRound(state, { ...state.counts, skipped: state.counts.skipped + 1 });
    case "undone":
      if (undoTarget(state) !== action.eventId) {
        return state;
      }
      return {
        ...state,
        saved: state.saved.slice(0, -1),
        counts: { ...state.counts, undone: state.counts.undone + 1 },
      };
    case "flagsToggled":
      return { ...state, flags: toggled(state.flags, action.toggle) };
    case "sessionDone":
      return { ...state, window: null, done: true };
  }
}

export function undoTarget(state: SessionState): string | null {
  return state.saved.at(-1) ?? null;
}

export function sampleExclusion(state: SessionState): { seed: string; round: number; exclude: Anchor[] } {
  return { seed: state.seed, round: state.round, exclude: state.shown };
}

export interface InlineFlags {
  toggleMixed: boolean;
  toggleUnsure: boolean;
  thumb: ThumbSide | null;
}

/** The flags a label answer is stored with: inline toggles flip the window's, an inline thumb side wins. */
export function submitFlags(
  flags: SessionFlags,
  inline: InlineFlags,
): { mixed: boolean; unsure: boolean; thumbPref: ThumbSide | null } {
  return {
    mixed: flags.mixed !== inline.toggleMixed,
    unsure: flags.unsure !== inline.toggleUnsure,
    thumbPref: inline.thumb ?? flags.thumb,
  };
}
