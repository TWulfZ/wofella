import { describe, expect, it } from "vitest";
import {
  type AudioBufferLike,
  type AudioBufferSourceNodeLike,
  type AudioContextLike,
  createAudioLoopClock,
  createSilentLoopClock,
  loopPosition,
} from "./audioClock";

const LOOP = { startMs: 1000, endMs: 2000 };

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
});

class FakeSource implements AudioBufferSourceNodeLike {
  buffer: AudioBufferLike | null = null;
  loop = false;
  loopStart = 0;
  loopEnd = 0;
  starts: { when: number | undefined; offset: number | undefined }[] = [];
  stops = 0;
  destinations: unknown[] = [];
  disconnects = 0;
  connect(destination: object): void {
    this.destinations.push(destination);
  }
  disconnect(): void {
    this.disconnects++;
  }
  start(when?: number, offset?: number): void {
    this.starts.push({ when, offset });
  }
  stop(): void {
    this.stops++;
  }
}

interface FakeContext extends AudioContextLike {
  currentTime: number;
  state: string;
  sources: FakeSource[];
  resumes: number;
}

function fakeContext(latency: { outputLatency?: number; baseLatency?: number } = {}): FakeContext {
  const ctx: FakeContext = {
    ...latency,
    currentTime: 10,
    state: "running",
    destination: { name: "speakers" },
    sources: [],
    resumes: 0,
    createBufferSource() {
      const source = new FakeSource();
      ctx.sources.push(source);
      return source;
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

function lastSource(ctx: FakeContext): FakeSource {
  const source = ctx.sources.at(-1);
  if (source === undefined) {
    throw new Error("no source node was created");
  }
  return source;
}

describe("createAudioLoopClock", () => {
  it("starts paused at the loop start plus the offset, without touching the context", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 20);
    expect(clock.playing).toBe(false);
    expect(clock.nowMs()).toBe(1020);
    expect(ctx.sources).toEqual([]);
  });

  it("plays a looping source over the loop, from the loop start", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0);
    clock.play();
    const source = lastSource(ctx);
    expect(clock.playing).toBe(true);
    expect(source.buffer).toBe(BUFFER);
    expect(source.loop).toBe(true);
    expect(source.loopStart).toBe(1);
    expect(source.loopEnd).toBe(2);
    expect(source.destinations).toEqual([ctx.destination]);
    expect(source.starts).toEqual([{ when: 0, offset: 1 }]);
  });

  it("reports loop position minus output latency plus offset, wrapping at the loop end", () => {
    const ctx = fakeContext({ outputLatency: 0.05, baseLatency: 0.01 });
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 20);
    clock.play();
    ctx.currentTime = 10.3;
    expect(clock.nowMs()).toBeCloseTo(1000 + 250 + 20);
    ctx.currentTime = 11.3;
    expect(clock.nowMs()).toBeCloseTo(1000 + 250 + 20);
  });

  it("falls back to base latency, then to none", () => {
    const base = fakeContext({ baseLatency: 0.01 });
    const withBase = createAudioLoopClock(base, BUFFER, LOOP, 0);
    withBase.play();
    base.currentTime = 10.3;
    expect(withBase.nowMs()).toBeCloseTo(1290);

    const bare = fakeContext();
    const withNone = createAudioLoopClock(bare, BUFFER, LOOP, 0);
    withNone.play();
    bare.currentTime = 10.3;
    expect(withNone.nowMs()).toBeCloseTo(1300);
  });

  it("holds the start while the first audio is still in the output latency", () => {
    const ctx = fakeContext({ outputLatency: 0.05 });
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0);
    clock.play();
    ctx.currentTime = 10.02;
    expect(clock.nowMs()).toBe(1000);
  });

  it("pauses by stopping the node and resumes from the same position on a new node", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0);
    clock.play();
    ctx.currentTime = 10.4;
    clock.pause();
    const first = lastSource(ctx);
    expect(clock.playing).toBe(false);
    expect(first.stops).toBe(1);
    expect(first.disconnects).toBe(1);

    ctx.currentTime = 50;
    expect(clock.nowMs()).toBeCloseTo(1400);

    clock.play();
    const second = lastSource(ctx);
    expect(second).not.toBe(first);
    expect(second.starts).toHaveLength(1);
    expect(second.starts[0]?.offset).toBeCloseTo(1.4);
    ctx.currentTime = 50.1;
    expect(clock.nowMs()).toBeCloseTo(1500);
  });

  it("ignores play while playing and pause while paused", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0);
    clock.pause();
    clock.play();
    clock.play();
    expect(ctx.sources).toHaveLength(1);
  });

  it("clamps the loop end to the buffer, since WebAudio ignores a loop past its end", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, { duration: 1.5 }, LOOP, 0);
    clock.play();
    expect(lastSource(ctx).loopEnd).toBe(1.5);
    ctx.currentTime = 10.6;
    expect(clock.nowMs()).toBeCloseTo(1100);
  });

  it("resumes a suspended context on play", () => {
    const ctx = fakeContext();
    ctx.state = "suspended";
    createAudioLoopClock(ctx, BUFFER, LOOP, 0).play();
    expect(ctx.resumes).toBe(1);
  });

  it("applies a new offset without restarting the audio", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0);
    clock.play();
    ctx.currentTime = 10.3;
    clock.setOffsetMs(-30);
    expect(clock.nowMs()).toBeCloseTo(1270);
    expect(ctx.sources).toHaveLength(1);
  });

  it("stops for good on dispose", () => {
    const ctx = fakeContext();
    const clock = createAudioLoopClock(ctx, BUFFER, LOOP, 0);
    clock.play();
    clock.dispose();
    expect(lastSource(ctx).stops).toBe(1);
    expect(clock.playing).toBe(false);
    clock.play();
    expect(ctx.sources).toHaveLength(1);
    expect(clock.playing).toBe(false);
  });
});

describe("createSilentLoopClock", () => {
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
