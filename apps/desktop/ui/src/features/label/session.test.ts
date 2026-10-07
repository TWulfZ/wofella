import { describe, expect, it } from "vitest";
import {
  canSave,
  canUndo,
  currentEntry,
  EMPTY_ANSWER,
  initialSession,
  isSampling,
  nowPlayingRequest,
  randomRequest,
  sampleExclusion,
  type SessionAction,
  type SessionState,
  sessionReducer,
  submitPayload,
  undoTarget,
  type WindowOrigin,
} from "./session";
import type { Anchor, LabelWindow } from "./types";

function anchor(md5: string, t0Ms = 1000, t1Ms = 5000): Anchor {
  return { md5, t0Ms, t1Ms, cols: [1, 2, 3, 4, 5, 6, 7] };
}

function labelWindow(md5: string): LabelWindow {
  return {
    anchor: anchor(md5),
    title: `Title ${md5}`,
    artist: "Artist",
    version: "Hard",
    level: null,
    stratum: "dan_07/nps_2",
    played: false,
  };
}

function run(state: SessionState, ...actions: SessionAction[]): SessionState {
  return actions.reduce(sessionReducer, state);
}

function planned(state: SessionState, md5: string): SessionState {
  return sessionReducer(state, {
    type: "windowLoaded",
    window: labelWindow(md5),
    origin: { kind: "plan", round: state.round },
  });
}

function loadedAs(state: SessionState, md5: string, origin: WindowOrigin): SessionState {
  return sessionReducer(state, { type: "windowLoaded", window: labelWindow(md5), origin });
}

function saved(state: SessionState, eventId: string): SessionState {
  return sessionReducer(state, { type: "submitted", eventId, answer: state.answer });
}

describe("sessionReducer history", () => {
  it("starts empty, waiting for the first planned window", () => {
    const s = initialSession("42");
    expect(s.history).toEqual([]);
    expect(s.cursor).toBe(0);
    expect(isSampling(s)).toBe(true);
    expect(currentEntry(s)).toBeNull();
    expect(s.answer).toEqual(EMPTY_ANSWER);
    expect(s.counts).toEqual({ labelled: 0, skipped: 0, undone: 0 });
    expect(sampleExclusion(s)).toEqual({ seed: "42", round: 0, exclude: [] });
  });

  it("appends a planned window, advancing the round and recording it as shown", () => {
    const s = planned(initialSession("42"), "a");
    expect(currentEntry(s)).toEqual({
      window: labelWindow("a"),
      origin: { kind: "plan", round: 0 },
      status: { kind: "pending" },
    });
    expect(isSampling(s)).toBe(false);
    expect(s.round).toBe(1);
    expect(s.shown).toEqual([anchor("a")]);
  });

  it("walks back and forth with Previous and Next, sampling only past the newest window", () => {
    let s = planned(initialSession("42"), "a");
    s = sessionReducer(s, { type: "moved", to: "next" });
    expect(isSampling(s)).toBe(true);
    expect(sampleExclusion(s)).toEqual({ seed: "42", round: 1, exclude: [anchor("a")] });
    s = planned(s, "b");
    s = sessionReducer(s, { type: "moved", to: "previous" });
    expect(currentEntry(s)?.window.anchor.md5).toBe("a");
    expect(currentEntry(s)?.status).toEqual({ kind: "pending" });
    s = sessionReducer(s, { type: "moved", to: "previous" });
    expect(s.cursor).toBe(0);
    s = sessionReducer(s, { type: "moved", to: "next" });
    expect(currentEntry(s)?.window.anchor.md5).toBe("b");
    expect(s.round).toBe(2);
    expect(s.counts.skipped).toBe(0);
  });

  it("does not advance the stratified round for random or now-playing windows, but excludes them", () => {
    let s = planned(initialSession("42"), "a");
    s = loadedAs(s, "r", { kind: "random" });
    expect(s.round).toBe(1);
    expect(s.randomRound).toBe(1);
    expect(randomRequest(s)).toEqual({ seed: "42", round: 1, exclude: [anchor("a"), anchor("r")] });
    s = loadedAs(s, "n", { kind: "nowPlaying", source: "lastReplay" });
    expect(s.round).toBe(1);
    expect(s.history.map((e) => e.origin.kind)).toEqual(["plan", "random", "nowPlaying"]);
    expect(currentEntry(s)?.window.anchor.md5).toBe("n");
    expect(sampleExclusion(s).exclude).toEqual([anchor("a"), anchor("r"), anchor("n")]);
    expect(nowPlayingRequest(s)).toEqual({ exclude: [anchor("a"), anchor("r"), anchor("n")] });
  });

  it("jumps to a window already in the history instead of adding it twice", () => {
    let s = planned(initialSession("42"), "a");
    s = loadedAs(s, "n", { kind: "nowPlaying", source: "osuWindow" });
    s = planned(sessionReducer(s, { type: "moved", to: "next" }), "b");
    s = loadedAs(s, "n", { kind: "nowPlaying", source: "osuWindow" });
    expect(s.history).toHaveLength(3);
    expect(s.cursor).toBe(1);
    expect(s.shown).toHaveLength(3);
  });

  it("saves the current answer, keeps it on the entry and moves on", () => {
    let s = planned(initialSession("42"), "a");
    s = run(s, { type: "patternToggled", id: "p.js" }, { type: "flagsToggled", toggle: "mixed" });
    s = saved(s, "01A");
    expect(s.history[0]?.status).toEqual({
      kind: "saved",
      eventId: "01A",
      answer: { ...EMPTY_ANSWER, patterns: ["p.js"], mixed: true },
    });
    expect(s.saved).toEqual(["01A"]);
    expect(s.counts.labelled).toBe(1);
    expect(isSampling(s)).toBe(true);
    expect(s.answer).toEqual(EMPTY_ANSWER);
  });

  it("skips a pending window once, counting it, and moves to the next one already shown", () => {
    let s = planned(initialSession("42"), "a");
    s = planned(sessionReducer(s, { type: "moved", to: "next" }), "b");
    s = run(s, { type: "moved", to: "previous" }, { type: "skipped" });
    expect(s.history[0]?.status).toEqual({ kind: "skipped" });
    expect(s.counts.skipped).toBe(1);
    expect(currentEntry(s)?.window.anchor.md5).toBe("b");
    s = run(s, { type: "moved", to: "previous" }, { type: "skipped" });
    expect(s.counts.skipped).toBe(1);
  });

  it("lets a skipped window be labelled when revisited", () => {
    let s = run(planned(initialSession("42"), "a"), { type: "skipped" });
    s = planned(s, "b");
    s = run(s, { type: "moved", to: "previous" }, { type: "noPatternToggled" });
    expect(canSave(s)).toBe(true);
    s = saved(s, "01A");
    expect(s.history[0]?.status).toMatchObject({ kind: "saved", eventId: "01A" });
    expect(currentEntry(s)?.window.anchor.md5).toBe("b");
  });

  it("keeps a saved window read-only and refuses to skip it", () => {
    let s = saved(run(planned(initialSession("42"), "a"), { type: "noPatternToggled" }), "01A");
    s = run(planned(s, "b"), { type: "moved", to: "previous" });
    const before = s;
    s = run(s, { type: "patternToggled", id: "p.js" }, { type: "flagsToggled", toggle: "unsure" }, { type: "skipped" });
    expect(s.answer).toEqual(before.answer);
    expect(s.cursor).toBe(0);
    expect(s.counts.skipped).toBe(0);
    expect(canSave(s)).toBe(false);
  });

  it("undoes only the newest saved event, turning its window back to pending", () => {
    let s = initialSession("42");
    for (const id of ["a", "b"]) {
      s = saved(run(planned(s, id), { type: "noPatternToggled" }), id.toUpperCase());
    }
    s = run(planned(s, "c"), { type: "moved", to: "previous" }, { type: "moved", to: "previous" });
    expect(canUndo(s)).toBe(false);
    expect(sessionReducer(s, { type: "undone", eventId: "A" })).toBe(s);
    s = sessionReducer(s, { type: "moved", to: "next" });
    expect(undoTarget(s)).toBe("B");
    expect(canUndo(s)).toBe(true);
    s = sessionReducer(s, { type: "undone", eventId: "B" });
    expect(s.saved).toEqual(["A"]);
    expect(s.history[1]?.status).toEqual({ kind: "pending" });
    expect(s.counts.undone).toBe(1);
    expect(s.cursor).toBe(1);
  });
});

describe("sessionReducer answer", () => {
  it("toggles patterns in pick order and clears no-pattern when one is picked", () => {
    let s = run(planned(initialSession("1"), "a"), { type: "noPatternToggled" });
    expect(s.answer.noPattern).toBe(true);
    s = run(s, { type: "patternToggled", id: "p.js" }, { type: "patternToggled", id: "p.mj" });
    expect(s.answer).toMatchObject({ patterns: ["p.js", "p.mj"], noPattern: false });
    s = run(s, { type: "patternToggled", id: "p.js" });
    expect(s.answer.patterns).toEqual(["p.mj"]);
    s = run(s, { type: "patternRemoved", id: "p.mj" }, { type: "patternRemoved", id: "p.mj" });
    expect(s.answer.patterns).toEqual([]);
  });

  it("adds a pattern from the picker without ever removing it", () => {
    let s = run(planned(initialSession("1"), "a"), { type: "noPatternToggled" });
    s = run(s, { type: "patternAdded", id: "p.js" }, { type: "patternAdded", id: "p.mj" });
    expect(s.answer).toMatchObject({ patterns: ["p.js", "p.mj"], noPattern: false });
    expect(sessionReducer(s, { type: "patternAdded", id: "p.js" }).answer.patterns).toEqual(["p.js", "p.mj"]);
  });

  it("drops the picked patterns when no-pattern is turned on", () => {
    const s = run(planned(initialSession("1"), "a"), { type: "patternToggled", id: "p.js" }, { type: "noPatternToggled" });
    expect(s.answer).toMatchObject({ patterns: [], noPattern: true });
  });

  it("toggles flags, the same thumb side again going back to neutral", () => {
    let s = planned(initialSession("1"), "a");
    s = run(s, { type: "flagsToggled", toggle: "mixed" }, { type: "flagsToggled", toggle: "unsure" });
    s = run(s, { type: "flagsToggled", toggle: "mixed" }, { type: "flagsToggled", toggle: "thumbLeft" });
    expect(s.answer).toMatchObject({ mixed: false, unsure: true, thumb: "left" });
    s = run(s, { type: "flagsToggled", toggle: "thumbRight" });
    expect(s.answer.thumb).toBe("right");
    s = run(s, { type: "flagsToggled", toggle: "thumbRight" });
    expect(s.answer.thumb).toBeNull();
  });

  it("is valid only with a pattern or no-pattern on a window that is not saved", () => {
    let s = planned(initialSession("1"), "a");
    expect(canSave(s)).toBe(false);
    s = run(s, { type: "flagsToggled", toggle: "mixed" });
    expect(canSave(s)).toBe(false);
    s = run(s, { type: "patternToggled", id: "p.js" });
    expect(canSave(s)).toBe(true);
    expect(canSave(initialSession("1"))).toBe(false);
  });

  it("builds the submit payload from the structured answer", () => {
    const s = run(
      planned(initialSession("1"), "a"),
      { type: "patternToggled", id: "p.lj" },
      { type: "flagsToggled", toggle: "unsure" },
      { type: "flagsToggled", toggle: "thumbRight" },
    );
    expect(submitPayload(s)).toEqual({
      anchor: anchor("a"),
      patterns: ["p.lj"],
      noPattern: false,
      mixed: false,
      unsure: true,
      thumbPref: "right",
    });
    expect(submitPayload(initialSession("1"))).toBeNull();
  });

  it("clears the answer, and resets it whenever the shown window changes", () => {
    let s = run(planned(initialSession("1"), "a"), { type: "patternToggled", id: "p.js" });
    expect(sessionReducer(s, { type: "answerCleared" }).answer).toEqual(EMPTY_ANSWER);
    s = run(planned(sessionReducer(s, { type: "moved", to: "next" }), "b"), { type: "patternToggled", id: "p.mj" });
    s = sessionReducer(s, { type: "moved", to: "previous" });
    expect(s.answer).toEqual(EMPTY_ANSWER);
  });

  it("keeps the answer across a reshape, which changes the anchor but not what was shown", () => {
    let s = run(planned(initialSession("1"), "a"), { type: "flagsToggled", toggle: "unsure" });
    const wider = anchor("a", 500, 5500);
    s = sessionReducer(s, { type: "windowReshaped", anchor: wider });
    expect(currentEntry(s)?.window.anchor).toEqual(wider);
    expect(currentEntry(s)?.window.title).toBe("Title a");
    expect(s.shown).toEqual([anchor("a")]);
    expect(s.answer.unsure).toBe(true);
    const empty = initialSession("1");
    expect(sessionReducer(empty, { type: "windowReshaped", anchor: wider })).toBe(empty);
  });

  it("keeps the history, undo and the other sources usable once the plan is finished", () => {
    let s = saved(run(planned(initialSession("1"), "a"), { type: "noPatternToggled" }), "01A");
    s = sessionReducer(s, { type: "sessionDone" });
    expect(currentEntry(s)).toBeNull();
    expect(sessionReducer(s, { type: "moved", to: "next" })).toBe(s);
    expect(sessionReducer(s, { type: "skipped" })).toBe(s);
    s = sessionReducer(s, { type: "moved", to: "previous" });
    expect(currentEntry(s)?.window.anchor.md5).toBe("a");
    expect(canUndo(s)).toBe(true);
    s = run(s, { type: "moved", to: "next" }, { type: "moved", to: "next" });
    expect(s.cursor).toBe(1);
    s = loadedAs(s, "r", { kind: "random" });
    expect(currentEntry(s)?.window.anchor.md5).toBe("r");
    s = sessionReducer(s, { type: "skipped" });
    expect(s.cursor).toBe(2);
    expect(isSampling(s)).toBe(false);
  });

  it("ends the session", () => {
    const s = sessionReducer(planned(initialSession("1"), "a"), { type: "sessionDone" });
    expect(s.done).toBe(true);
    expect(isSampling(s)).toBe(false);
  });
});
