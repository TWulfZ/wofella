import { describe, expect, it } from "vitest";
import {
  initialSession,
  sampleExclusion,
  type SessionState,
  sessionReducer,
  submitFlags,
  undoTarget,
} from "./session";
import type { Anchor, LabelWindow } from "./types";

function anchor(md5: string, t0Ms = 1000, t1Ms = 5000): Anchor {
  return { md5, t0Ms, t1Ms, cols: [1, 2, 3, 4, 5, 6, 7] };
}

function labelWindow(md5: string): LabelWindow {
  return {
    anchor: anchor(md5),
    title: "Title",
    artist: "Artist",
    version: "Hard",
    level: null,
    stratum: "dan_07/nps_2",
    played: false,
  };
}

function loaded(state: SessionState, md5: string): SessionState {
  return sessionReducer(state, { type: "windowLoaded", window: labelWindow(md5) });
}

describe("sessionReducer", () => {
  it("starts at round 0 with nothing shown or saved", () => {
    expect(initialSession("42")).toEqual({
      seed: "42",
      round: 0,
      shown: [],
      saved: [],
      flags: { mixed: false, unsure: false, thumb: null },
      window: null,
      done: false,
      counts: { labelled: 0, skipped: 0, undone: 0 },
    });
  });

  it("records every loaded window as shown and resets the flags", () => {
    let s = sessionReducer(initialSession("1"), { type: "flagsToggled", toggle: "mixed" });
    s = loaded(s, "a");
    expect(s.window?.anchor.md5).toBe("a");
    expect(s.shown).toEqual([anchor("a")]);
    expect(s.flags).toEqual({ mixed: false, unsure: false, thumb: null });
    s = loaded(sessionReducer(s, { type: "skipped" }), "b");
    expect(s.shown.map((a) => a.md5)).toEqual(["a", "b"]);
    expect(sampleExclusion(s)).toEqual({ seed: "1", round: 1, exclude: s.shown });
  });

  it("reshapes the current anchor without adding it to shown and keeps the flags", () => {
    let s = loaded(initialSession("1"), "a");
    s = sessionReducer(s, { type: "flagsToggled", toggle: "unsure" });
    const wider = anchor("a", 500, 5500);
    s = sessionReducer(s, { type: "windowReshaped", anchor: wider });
    expect(s.window?.anchor).toEqual(wider);
    expect(s.window?.title).toBe("Title");
    expect(s.shown).toEqual([anchor("a")]);
    expect(s.flags.unsure).toBe(true);
  });

  it("ignores a reshape with no window", () => {
    const s = initialSession("1");
    expect(sessionReducer(s, { type: "windowReshaped", anchor: anchor("a") })).toBe(s);
  });

  it("saves a submitted event and moves to the next round", () => {
    let s = loaded(initialSession("1"), "a");
    s = sessionReducer(s, { type: "submitted", eventId: "01A" });
    expect(s.saved).toEqual(["01A"]);
    expect(s.round).toBe(1);
    expect(s.window).toBeNull();
    expect(s.counts.labelled).toBe(1);
  });

  it("counts a skip without saving anything", () => {
    let s = loaded(initialSession("1"), "a");
    s = sessionReducer(s, { type: "skipped" });
    expect(s.saved).toEqual([]);
    expect(s.round).toBe(1);
    expect(s.window).toBeNull();
    expect(s.counts.skipped).toBe(1);
  });

  it("pops the last saved id only once its undo succeeded", () => {
    let s = initialSession("1");
    for (const id of ["a", "b"]) {
      s = sessionReducer(loaded(s, id), { type: "submitted", eventId: id });
    }
    expect(undoTarget(s)).toBe("b");
    // A failed undo dispatches nothing, so the id stays and can be retried.
    expect(undoTarget(s)).toBe("b");
    s = sessionReducer(s, { type: "undone", eventId: "b" });
    expect(s.saved).toEqual(["a"]);
    expect(s.counts.undone).toBe(1);
    expect(undoTarget(s)).toBe("a");
  });

  it("ignores a stale undo that is not the last saved id", () => {
    let s = sessionReducer(loaded(initialSession("1"), "a"), { type: "submitted", eventId: "a" });
    s = sessionReducer(s, { type: "undone", eventId: "zz" });
    expect(s.saved).toEqual(["a"]);
    expect(s.counts.undone).toBe(0);
    expect(undoTarget(initialSession("1"))).toBeNull();
  });

  it("toggles flags, the same thumb side again going back to neutral", () => {
    let s = loaded(initialSession("1"), "a");
    s = sessionReducer(s, { type: "flagsToggled", toggle: "mixed" });
    s = sessionReducer(s, { type: "flagsToggled", toggle: "unsure" });
    s = sessionReducer(s, { type: "flagsToggled", toggle: "mixed" });
    expect(s.flags).toEqual({ mixed: false, unsure: true, thumb: null });
    s = sessionReducer(s, { type: "flagsToggled", toggle: "thumbLeft" });
    expect(s.flags.thumb).toBe("left");
    s = sessionReducer(s, { type: "flagsToggled", toggle: "thumbRight" });
    expect(s.flags.thumb).toBe("right");
    s = sessionReducer(s, { type: "flagsToggled", toggle: "thumbRight" });
    expect(s.flags.thumb).toBeNull();
  });

  it("sets the thumb side outright, keeping the other flags", () => {
    let s = loaded(initialSession("1"), "a");
    s = sessionReducer(s, { type: "flagsToggled", toggle: "mixed" });
    s = sessionReducer(s, { type: "thumbSet", side: "right" });
    expect(s.flags).toEqual({ mixed: true, unsure: false, thumb: "right" });
    s = sessionReducer(s, { type: "thumbSet", side: "right" });
    expect(s.flags.thumb).toBe("right");
    s = sessionReducer(s, { type: "thumbSet", side: null });
    expect(s.flags).toEqual({ mixed: true, unsure: false, thumb: null });
  });

  it("ends the session", () => {
    const s = sessionReducer(loaded(initialSession("1"), "a"), { type: "sessionDone" });
    expect(s.done).toBe(true);
    expect(s.window).toBeNull();
  });
});

describe("submitFlags", () => {
  it("XORs the inline toggles with the window flags and lets an inline thumb win", () => {
    const flags = { mixed: true, unsure: false, thumb: "left" as const };
    expect(submitFlags(flags, { toggleMixed: true, toggleUnsure: true, thumb: null })).toEqual({
      mixed: false,
      unsure: true,
      thumbPref: "left",
    });
    expect(submitFlags(flags, { toggleMixed: false, toggleUnsure: false, thumb: "right" })).toEqual({
      mixed: true,
      unsure: false,
      thumbPref: "right",
    });
  });
});
