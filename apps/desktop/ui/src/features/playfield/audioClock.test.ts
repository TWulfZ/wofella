import { describe, expect, it } from "vitest";
import {
  type AudioBufferLike,
  type AudioBufferSourceNodeLike,
  type AudioContextLike,
  type AudioParamLike,
  createAudioLoopClock,
  createSilentLoopClock,
  type GainNodeLike,
  loopPosition,
} from "./audioClock";

const LOOP = { startMs: 1000, endMs: 2000 };
const NO_GAP = { fadeMs: 30, gapMs: 0 };
const SPLICE = { fadeMs: 30, gapMs: 150 };
const PERIOD_S = 1.15;

describe("loopPosition", () => {
  it("offsets elapsed time from the loop start", () => {
    expect(loopPosition(250, LOOP)).toBe(1250);
  });

  it("wraps around at the loop end", () => {
    expect(loopPosition(1000, LOOP)).toBe(1000);
    expect(loopPosition(2250, LOOP)).toBe(1250);
  });

  it("clamps negative elapsed time to the loop start", () => {
    expect(loopPosition(-40, LOOP)).toBe(1000);
  });

  it("stays at the start of an empty or inverted loop", () => {
    expect(loopPosition(500, { startMs: 1000, endMs: 1000 })).toBe(1000);
    expect(loopPosition(500, { startMs: 1000, endMs: 900 })).toBe(1000);
  });

  it("holds the loop start through the gap, then wraps on the period of length plus gap", () => {
    expect(loopPosition(999, LOOP, 150)).toBe(1999);
    expect(loopPosition(1000, LOOP, 150)).toBe(1000);
    expect(loopPosition(1149, LOOP, 150)).toBe(1000);
    expect(loopPosition(1200, LOOP, 150)).toBe(1050);
  });
});

type GainEvent = readonly [kind: "set" | "ramp", value: number, atS: number];

class FakeParam implements AudioParamLike {
  value = 1;
  events: GainEvent[] = [];
  setValueAtTime(value: number, atS: number): void {
    this.events.push(["set", value, atS]);
  }
  linearRampToValueAtTime(value: number, atS: number): void {
    this.events.push(["ramp", value, atS]);
  }
}

class FakeGain implements GainNodeLike {
  readonly gain = new FakeParam();
  destinations: unknown[] = [];
  disconnects = 0;
  connect(destination: object): void {
    this.destinations.push(destination);
  }
  disconnect(): void {
    this.disconnects++;
  }
}

class FakeSource implements AudioBufferSourceNodeLike {
  buffer: AudioBufferLike | null = null;
  readonly playbackRate = new FakeParam();
  onended: ((ev: Event) => unknown) | null = null;
  starts: { when: number | undefined; offset: number | undefined; duration: number | undefined }[] = [];
  /** The context time of each start call: WebAudio plays a `when` already in the past right away. */
  calledAt: number[] = [];
  now: () => number = () => 0;
  stops = 0;
  ended = false;
  destinations: unknown[] = [];
  disconnects = 0;
  connect(destination: object): void {
    this.destinations.push(destination);
  }
  disconnect(): void {
    this.disconnects++;
  }
  start(when?: number, offset?: number, duration?: number): void {
    this.starts.push({ when, offset, duration });
    this.calledAt.push(this.now());
  }
  stop(): void {
    this.stops++;
  }
  get live(): boolean {
    return this.starts.length > 0 && this.stops === 0 && !this.ended;
  }
  /** What WebAudio does once the scheduled duration has played. */
  end(): void {
    this.ended = true;
    this.onended?.(new Event("ended"));
  }
}

interface FakeContext extends AudioContextLike {
  currentTime: number;
  state: string;
  sources: FakeSource[];
  gains: FakeGain[];
  resumes: number;
}

function fakeContext(latency: { outputLatency?: number; baseLatency?: number } = {}): FakeContext {
  const ctx: FakeContext = {
    ...latency,
    currentTime: 10,
    state: "running",
    destination: { name: "speakers" },
    sources: [],
    gains: [],
    resumes: 0,
    createBufferSource() {
      const source = new FakeSource();
      source.now = () => ctx.currentTime;
      ctx.sources.push(source);
      return source;
    },
    createGain() {
      const gain = new FakeGain();
      ctx.gains.push(gain);
      return gain;
    },
    async resume() {
      ctx.resumes++;
      ctx.state = "running";
      return Promise.resolve();
    },
    async decodeAudioData() {
      return Promise.reject(new Error("unused"));
    },
  };
  return ctx;
}

const BUFFER = { duration: 120 };

function nth<T>(items: readonly T[], index: number): T {
  const item = items.at(index);
  if (item === undefined) {
    throw new Error(`no item at ${index}`);
  }
  return item;
}

function live(ctx: FakeContext): FakeSource[] {
  return ctx.sources.filter((s) => s.live);
}

function rounded(events: readonly GainEvent[]): GainEvent[] {
  return events.map(([kind, value, atS]) => [kind, value, Math.round(atS * 1e6) / 1e6]);
}

describe("createAudioLoopClock", () => {
  it("starts paused at the loop start plus the offset, without touching the context", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 20, SPLICE);
    expect(clock.playing).toBe(false);
    expect(clock.nowMs()).toBe(1020);
    expect(ctx.sources).toEqual([]);
  });

  it("plays one-shot iterations through their own gain node: the current one and the next, ahead of time", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.play();
    expect(clock.playing).toBe(true);
    expect(ctx.sources).toHaveLength(2);
    const [first, next] = [nth(ctx.sources, 0), nth(ctx.sources, 1)];
    expect(first.buffer).toBe(BUFFER);
    expect(first.starts).toEqual([{ when: 10, offset: 1, duration: 1 }]);
    expect(next.starts).toHaveLength(1);
    expect(next.starts[0]?.when).toBeCloseTo(10 + PERIOD_S);
    expect(next.starts[0]?.offset).toBe(1);
    expect(next.starts[0]?.duration).toBe(1);
    ctx.sources.forEach((source, i) => {
      const gain = nth(ctx.gains, i);
      expect(source.destinations).toEqual([gain]);
      expect(gain.destinations).toEqual([ctx.destination]);
    });
  });

  it("fades each iteration in at its start and out at its end", () => {
    const ctx = fakeContext();
    createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE).play();
    expect(rounded(nth(ctx.gains, 0).gain.events)).toEqual([
      ["set", 0, 10],
      ["ramp", 1, 10.03],
      ["set", 1, 10.97],
      ["ramp", 0, 11],
    ]);
    expect(rounded(nth(ctx.gains, 1).gain.events)).toEqual([
      ["set", 0, 11.15],
      ["ramp", 1, 11.18],
      ["set", 1, 12.12],
      ["ramp", 0, 12.15],
    ]);
  });

  it("fades a remainder shorter than two fades over half its length each way", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.play();
    ctx.currentTime = 10.98;
    clock.pause();
    ctx.currentTime = 20;
    clock.play();
    const resumed = nth(ctx.sources, 2);
    expect(resumed.starts[0]?.offset).toBeCloseTo(1.98);
    expect(resumed.starts[0]?.duration).toBeCloseTo(0.02);
    expect(rounded(nth(ctx.gains, 2).gain.events)).toEqual([
      ["set", 0, 20],
      ["ramp", 1, 20.01],
      ["set", 1, 20.01],
      ["ramp", 0, 20.02],
    ]);
  });

  it("schedules the next iteration whenever one ends, keeping one playing and one ahead over ten iterations", () => {
    const ctx = fakeContext();
    createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE).play();
    for (let i = 0; i < 10; i++) {
      const [playing, ahead] = live(ctx);
      expect(live(ctx)).toHaveLength(2);
      if (playing === undefined || ahead === undefined) {
        throw new Error("expected two live sources");
      }
      expect(playing.starts[0]?.when).toBeCloseTo(10 + i * PERIOD_S);
      expect(ahead.starts[0]?.when).toBeCloseTo(10 + (i + 1) * PERIOD_S);
      ctx.currentTime = 10 + i * PERIOD_S + 1;
      playing.end();
      expect(playing.disconnects).toBe(1);
      expect(nth(ctx.gains, ctx.sources.indexOf(playing)).disconnects).toBe(1);
      expect(live(ctx).length).toBeLessThanOrEqual(2);
    }
    expect(ctx.sources).toHaveLength(12);
    expect(nth(ctx.sources, -1).starts[0]?.when).toBeCloseTo(10 + 11 * PERIOD_S);
  });

  it("skips the passes a main-thread stall left in the past, so no two sources overlap and the clock still matches", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.play();
    // Both ended events arrive late, after the context ran on for more than a period.
    ctx.currentTime = 10 + 2.5 * PERIOD_S;
    nth(ctx.sources, 0).end();
    nth(ctx.sources, 1).end();
    for (let i = 0; i < 4; i++) {
      const [playing] = live(ctx);
      if (playing === undefined) {
        throw new Error("expected a live source");
      }
      ctx.currentTime = Math.max(ctx.currentTime, (playing.starts[0]?.when ?? 0) + 1);
      playing.end();
    }
    const intervals = ctx.sources.map((source) => {
      const { when = 0, offset = 0, duration = 0 } = source.starts[0] ?? {};
      const at = Math.max(when, nth(source.calledAt, 0));
      return { at, end: at + duration, offsetMs: offset * 1000 };
    });
    const sorted = [...intervals].sort((a, b) => a.at - b.at);
    for (let i = 1; i < sorted.length; i++) {
      expect(nth(sorted, i).at).toBeGreaterThanOrEqual(nth(sorted, i - 1).end - 1e-9);
    }
    for (const { at, offsetMs } of intervals) {
      ctx.currentTime = at;
      expect(clock.nowMs()).toBeCloseTo(offsetMs);
    }
  });

  it("holds the clock at the loop start through the gap", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 20, SPLICE);
    clock.play();
    ctx.currentTime = 10.99;
    expect(clock.nowMs()).toBeCloseTo(1990 + 20);
    ctx.currentTime = 11.05;
    expect(clock.nowMs()).toBeCloseTo(1000 + 20);
    ctx.currentTime = 11.2;
    expect(clock.nowMs()).toBeCloseTo(1050 + 20);
  });

  it("pausing in the gap stops every source and resumes at the loop start", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.play();
    ctx.currentTime = 11;
    nth(ctx.sources, 0).end();
    ctx.currentTime = 11.05;
    clock.pause();
    expect(live(ctx)).toEqual([]);
    expect(ctx.sources.every((s) => s.disconnects === 1)).toBe(true);
    expect(ctx.gains.every((g) => g.disconnects === 1)).toBe(true);
    expect(clock.nowMs()).toBe(1000);

    ctx.currentTime = 30;
    clock.play();
    const resumed = nth(ctx.sources, -2);
    expect(resumed.starts).toEqual([{ when: 30, offset: 1, duration: 1 }]);
  });

  it("pausing mid-iteration stops the playing and the scheduled source, and a late ended event schedules nothing", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.play();
    ctx.currentTime = 10.4;
    clock.pause();
    const [first, next] = [nth(ctx.sources, 0), nth(ctx.sources, 1)];
    expect([first.stops, next.stops]).toEqual([1, 1]);
    expect([first.disconnects, next.disconnects]).toEqual([1, 1]);
    expect(ctx.gains.map((g) => g.disconnects)).toEqual([1, 1]);
    first.end();
    expect(ctx.sources).toHaveLength(2);
  });

  it("reports loop position minus output latency plus offset, wrapping at the loop end", () => {
    const ctx = fakeContext({ outputLatency: 0.05, baseLatency: 0.01 });
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 20, NO_GAP);
    clock.play();
    ctx.currentTime = 10.3;
    expect(clock.nowMs()).toBeCloseTo(1000 + 250 + 20);
    ctx.currentTime = 11.3;
    expect(clock.nowMs()).toBeCloseTo(1000 + 250 + 20);
  });

  it("falls back to base latency, then to none", () => {
    const base = fakeContext({ baseLatency: 0.01 });
    const withBase = createAudioLoopClock(base, BUFFER, LOOP, 0, NO_GAP);
    withBase.play();
    base.currentTime = 10.3;
    expect(withBase.nowMs()).toBeCloseTo(1290);

    const bare = fakeContext();
    const withNone = createAudioLoopClock(bare, BUFFER, LOOP, 0, NO_GAP);
    withNone.play();
    bare.currentTime = 10.3;
    expect(withNone.nowMs()).toBeCloseTo(1300);
  });

  it("holds the start while the first audio is still in the output latency", () => {
    const ctx = fakeContext({ outputLatency: 0.05 });
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.play();
    ctx.currentTime = 10.02;
    expect(clock.nowMs()).toBe(1000);
  });

  it("pauses by stopping the nodes and resumes from the same position on new nodes", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.play();
    ctx.currentTime = 10.4;
    clock.pause();
    expect(clock.playing).toBe(false);

    ctx.currentTime = 50;
    expect(clock.nowMs()).toBeCloseTo(1400);

    clock.play();
    expect(ctx.sources).toHaveLength(4);
    const resumed = nth(ctx.sources, 2);
    expect(resumed.starts).toHaveLength(1);
    expect(resumed.starts[0]?.when).toBe(50);
    expect(resumed.starts[0]?.offset).toBeCloseTo(1.4);
    expect(resumed.starts[0]?.duration).toBeCloseTo(0.6);
    expect(nth(ctx.sources, 3).starts[0]?.when).toBeCloseTo(50 - 0.4 + PERIOD_S);
    ctx.currentTime = 50.1;
    expect(clock.nowMs()).toBeCloseTo(1500);
  });

  it("resumes from the scheduled position, not the heard one, so no latency's worth of audio plays twice", () => {
    const ctx = fakeContext({ outputLatency: 0.2 });
    const clock = createAudioLoopClock(ctx, BUFFER, { startMs: 0, endMs: 10000 }, 0, SPLICE);
    clock.play();
    ctx.currentTime = 13;
    clock.pause();
    expect(clock.nowMs()).toBeCloseTo(2800);
    clock.play();
    expect(nth(ctx.sources, 2).starts[0]?.offset).toBeCloseTo(3);
    expect(clock.nowMs()).toBeCloseTo(2800);
  });

  it("ignores play while playing and pause while paused", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.pause();
    clock.play();
    clock.play();
    expect(ctx.sources).toHaveLength(2);
  });

  it("clamps the loop end to the buffer, since a one-shot past it would end early and desync the clock", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, { duration: 1.5 }, LOOP, 0, SPLICE);
    clock.play();
    expect(nth(ctx.sources, 0).starts[0]?.duration).toBe(0.5);
    expect(nth(ctx.sources, 1).starts[0]?.when).toBeCloseTo(10.65);
    ctx.currentTime = 10.75;
    expect(clock.nowMs()).toBeCloseTo(1100);
  });

  it.each([
    ["starts at the buffer end", { duration: 1 }],
    ["starts past the buffer end", { duration: 0.5 }],
  ])("runs a silent loop on the context clock, with the same gap, when the loop %s", (_, buffer) => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, buffer, LOOP, 20, SPLICE);
    expect(clock.nowMs()).toBe(1020);
    clock.play();
    expect(clock.playing).toBe(true);
    expect(ctx.sources).toEqual([]);
    ctx.currentTime = 10.3;
    expect(clock.nowMs()).toBeCloseTo(1320);
    ctx.currentTime = 11.05;
    expect(clock.nowMs()).toBeCloseTo(1020);
    ctx.currentTime = 11.45;
    expect(clock.nowMs()).toBeCloseTo(1320);
    clock.setOffsetMs(-30);
    expect(clock.nowMs()).toBeCloseTo(1270);
    clock.pause();
    ctx.currentTime = 50;
    expect(clock.playing).toBe(false);
    expect(clock.nowMs()).toBeCloseTo(1270);
  });

  it("resumes a suspended context for the silent loop too, since its time is the context's", () => {
    const ctx = fakeContext();
    ctx.state = "suspended";
    createAudioLoopClock(ctx, { duration: 0.5 }, LOOP, 0, SPLICE).play();
    expect(ctx.resumes).toBe(1);
  });

  it("resumes a suspended context on play", () => {
    const ctx = fakeContext();
    ctx.state = "suspended";
    createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE).play();
    expect(ctx.resumes).toBe(1);
  });

  it("applies a new offset without restarting the audio", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.play();
    ctx.currentTime = 10.3;
    clock.setOffsetMs(-30);
    expect(clock.nowMs()).toBeCloseTo(1270);
    expect(ctx.sources).toHaveLength(2);
  });

  it("stops every source for good on dispose", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE);
    clock.play();
    clock.dispose();
    expect(ctx.sources.map((s) => s.stops)).toEqual([1, 1]);
    expect(ctx.gains.map((g) => g.disconnects)).toEqual([1, 1]);
    expect(clock.playing).toBe(false);
    clock.play();
    expect(ctx.sources).toHaveLength(2);
    expect(clock.playing).toBe(false);
  });
});

describe("createAudioLoopClock at a playback rate", () => {
  it("plays the buffer faster and advances chart time by the rate, the gap staying in wall time", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, SPLICE, 2);
    clock.play();
    const [first, next] = [nth(ctx.sources, 0), nth(ctx.sources, 1)];
    expect(first.playbackRate.value).toBe(2);
    // Offset and duration are buffer time; the 1 s section takes 0.5 s of the context, then the 150 ms gap. Fades and
    // the gap are heard, so they keep their wall length.
    expect(first.starts).toEqual([{ when: 10, offset: 1, duration: 1 }]);
    expect(next.starts[0]?.when).toBeCloseTo(10.65);
    expect(rounded(nth(ctx.gains, 0).gain.events)).toEqual([
      ["set", 0, 10],
      ["ramp", 1, 10.03],
      ["set", 1, 10.47],
      ["ramp", 0, 10.5],
    ]);
    ctx.currentTime = 10.25;
    expect(clock.nowMs()).toBe(1500);
    ctx.currentTime = 10.6;
    expect(clock.nowMs()).toBe(1000);
    ctx.currentTime = 10.7;
    expect(clock.nowMs()).toBeCloseTo(1100);
  });

  it("keeps the audio offset in wall time: +30 ms at 1.5× leads by 45 ms of chart", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 30, NO_GAP, 1.5);
    expect(clock.nowMs()).toBeCloseTo(1045);
    clock.play();
    ctx.currentTime = 10.2;
    expect(clock.nowMs()).toBeCloseTo(1000 + 300 + 45);
    clock.setOffsetMs(-20);
    expect(clock.nowMs()).toBeCloseTo(1000 + 300 - 30);
  });

  it("keeps the offset in wall time on the silent fallback too", () => {
    const ctx = fakeContext();
    // The loop starts past the end of this buffer, so the clock runs silently on the context.
    const clock = createAudioLoopClock(ctx, { duration: 0.5 }, LOOP, 30, NO_GAP, 0.5);
    expect(clock.nowMs()).toBeCloseTo(1015);
  });

  it("resumes a slowed loop from where it paused", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0, NO_GAP, 0.5);
    clock.play();
    ctx.currentTime = 10.4;
    expect(clock.nowMs()).toBeCloseTo(1200);
    clock.pause();
    ctx.currentTime = 20;
    clock.play();
    const resumed = nth(ctx.sources, 2);
    expect(resumed.starts[0]?.offset).toBeCloseTo(1.2);
    expect(resumed.starts[0]?.duration).toBeCloseTo(0.8);
    ctx.currentTime = 20.2;
    expect(clock.nowMs()).toBeCloseTo(1300);
  });
});

describe("createSilentLoopClock", () => {
  it("advances chart time by the playback rate", () => {
    let now = 0;
    const clock = createSilentLoopClock(LOOP, () => now, 150, 1.5);
    clock.play();
    now = 200;
    expect(clock.nowMs()).toBe(1300);
    clock.pause();
    now = 5000;
    clock.play();
    now = 5100;
    expect(clock.nowMs()).toBe(1450);
  });

  it("runs the same loop on a wall clock", () => {
    let now = 5000;
    const clock = createSilentLoopClock(LOOP, () => now);
    expect(clock.playing).toBe(false);
    expect(clock.nowMs()).toBe(1000);

    clock.play();
    now = 5250;
    expect(clock.playing).toBe(true);
    expect(clock.nowMs()).toBe(1250);
    now = 6250;
    expect(clock.nowMs()).toBe(1250);
  });

  it("mirrors the audio period: holds the loop start through the gap", () => {
    let now = 0;
    const clock = createSilentLoopClock(LOOP, () => now, 150);
    clock.play();
    now = 1050;
    expect(clock.nowMs()).toBe(1000);
    now = 1200;
    expect(clock.nowMs()).toBe(1050);
  });

  it("pauses and resumes from the same position", () => {
    let now = 0;
    const clock = createSilentLoopClock(LOOP, () => now);
    clock.play();
    now = 400;
    clock.pause();
    now = 9000;
    expect(clock.nowMs()).toBe(1400);
    clock.play();
    now = 9100;
    expect(clock.nowMs()).toBe(1500);
  });

  it("stops for good on dispose", () => {
    let now = 0;
    const clock = createSilentLoopClock(LOOP, () => now);
    clock.play();
    clock.dispose();
    now = 300;
    clock.play();
    expect(clock.playing).toBe(false);
    expect(clock.nowMs()).toBe(1000);
  });
});
