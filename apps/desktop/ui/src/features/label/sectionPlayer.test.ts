import { describe, expect, it } from "vitest";
import { type AudioBufferLike, type AudioBufferSourceNodeLike, type AudioContextLike, SEEK_END_GUARD_MS } from "@/features/playfield";
import { SectionPlayer, type SectionPlayerDeps, skipWithin } from "./sectionPlayer";

class FakeSource implements AudioBufferSourceNodeLike {
  buffer: AudioBufferLike | null = null;
  readonly playbackRate = { value: 1 };
  onended: ((ev: Event) => unknown) | null = null;
  starts: { when: number | undefined; offset: number | undefined; duration: number | undefined }[] = [];
  stopped = 0;
  connect(): void {
    // Routing is not observed.
  }
  disconnect(): void {
    // Routing is not observed.
  }
  start(when?: number, offset?: number, duration?: number): void {
    this.starts.push({ when, offset, duration });
  }
  stop(): void {
    this.stopped++;
  }
}

const FAKE_GAIN = {
  gain: { setValueAtTime: () => undefined, linearRampToValueAtTime: () => undefined },
  connect: () => undefined,
  disconnect: () => undefined,
};

interface FakeContext extends AudioContextLike {
  currentTime: number;
  sources: FakeSource[];
  closed: number;
  close(): Promise<void>;
}

function fakeContext(): FakeContext {
  const ctx: FakeContext = {
    currentTime: 0,
    state: "running",
    destination: {},
    sources: [],
    closed: 0,
    createBufferSource() {
      const source = new FakeSource();
      ctx.sources.push(source);
      return source;
    },
    createGain() {
      return FAKE_GAIN;
    },
    async resume() {
      return Promise.resolve();
    },
    async decodeAudioData() {
      return Promise.reject(new Error("decode goes through deps.decode"));
    },
    async close() {
      ctx.closed++;
      return Promise.resolve();
    },
  };
  return ctx;
}

interface Pending {
  base64: string;
  resolve: (buffer: AudioBufferLike) => void;
  reject: (error: Error) => void;
}

function harness(overrides: Partial<SectionPlayerDeps> = {}) {
  const contexts: FakeContext[] = [];
  const decodes: Pending[] = [];
  const now = { ms: 0 };
  const player = new SectionPlayer({
    createContext: () => {
      const ctx = fakeContext();
      contexts.push(ctx);
      return ctx;
    },
    decode: async (_ctx, base64) =>
      new Promise<AudioBufferLike>((resolve, reject) => {
        decodes.push({ base64, resolve, reject });
      }),
    now: () => now.ms,
    splice: SPLICE,
    ...overrides,
  });
  return { player, contexts, decodes, now };
}

const LOOP = { startMs: 1000, endMs: 3000 };
const DATA = { kind: "data", base64: "SUQz" } as const;
const BUFFER = { duration: 120 };
const SPLICE = { fadeMs: 30, gapMs: 150 };

async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

function onlyContext(contexts: FakeContext[]): FakeContext {
  expect(contexts).toHaveLength(1);
  const [ctx] = contexts;
  if (ctx === undefined) {
    throw new Error("no context");
  }
  return ctx;
}

describe("SectionPlayer", () => {
  it("stays idle, with no audio context, until the first play", () => {
    const { player, contexts, decodes } = harness();
    player.setSection("a", DATA, LOOP);
    expect(player.getSnapshot()).toEqual({ clock: null, playing: false, loading: false, notice: null });
    expect(contexts).toEqual([]);
    expect(decodes).toEqual([]);
  });

  it("creates the context on play, decodes, then loops the section on the audio", async () => {
    const { player, contexts, decodes } = harness();
    player.setSection("a", DATA, LOOP);
    player.play();
    const ctx = onlyContext(contexts);
    expect(player.getSnapshot()).toMatchObject({ clock: null, playing: true, loading: true });
    expect(decodes.map((d) => d.base64)).toEqual(["SUQz"]);

    decodes[0]?.resolve(BUFFER);
    await settle();
    const { clock, loading } = player.getSnapshot();
    expect(loading).toBe(false);
    expect(clock?.playing).toBe(true);
    expect(ctx.sources.map((s) => s.starts)).toEqual([
      [{ when: 0, offset: 1, duration: 2 }],
      [{ when: 2.15, offset: 1, duration: 2 }],
    ]);
  });

  it("keeps the decoded buffer across a window move within the same chart and restarts the loop playing", async () => {
    const { player, contexts, decodes } = harness();
    player.setSection("a", DATA, LOOP);
    player.play();
    decodes[0]?.resolve(BUFFER);
    await settle();
    const first = player.getSnapshot().clock;

    player.setSection("a", DATA, { startMs: 2000, endMs: 4000 });
    const ctx = onlyContext(contexts);
    expect(decodes).toHaveLength(1);
    const second = player.getSnapshot().clock;
    expect(second).not.toBe(first);
    expect(first?.playing).toBe(false);
    expect(second?.playing).toBe(true);
    expect(ctx.sources.at(-2)?.starts[0]?.offset).toBe(2);
  });

  it("decodes a new chart and ignores a decode that finishes after the chart changed", async () => {
    const { player, decodes } = harness();
    player.setSection("a", DATA, LOOP);
    player.play();
    player.setSection("b", { kind: "data", base64: "AAAA" }, LOOP);
    expect(decodes.map((d) => d.base64)).toEqual(["SUQz", "AAAA"]);

    decodes[0]?.resolve(BUFFER);
    await settle();
    expect(player.getSnapshot()).toMatchObject({ clock: null, loading: true });

    decodes[1]?.resolve(BUFFER);
    await settle();
    expect(player.getSnapshot().clock?.playing).toBe(true);
  });

  it("waits for pending audio, then decodes once it arrives", async () => {
    const { player, decodes } = harness();
    player.setSection("a", { kind: "pending" }, LOOP);
    player.play();
    expect(player.getSnapshot()).toMatchObject({ clock: null, loading: true, notice: null });
    player.setSection("a", DATA, LOOP);
    decodes[0]?.resolve(BUFFER);
    await settle();
    expect(player.getSnapshot().clock?.playing).toBe(true);
  });

  it("runs a silent clock with a notice when the chart has no audio", () => {
    const { player, now } = harness();
    player.setSection("a", { kind: "missing" }, LOOP);
    expect(player.getSnapshot().notice).toBe("unavailable");
    player.play();
    const { clock } = player.getSnapshot();
    expect(clock?.playing).toBe(true);
    now.ms = 250;
    expect(clock?.nowMs()).toBe(1250);
  });

  it("gives the silent clock the same splice gap as the audio", () => {
    const { player, now } = harness();
    player.setSection("a", { kind: "missing" }, LOOP);
    player.play();
    const { clock } = player.getSnapshot();
    now.ms = 2050;
    expect(clock?.nowMs()).toBe(1000);
    now.ms = 2250;
    expect(clock?.nowMs()).toBe(1100);
  });

  it("falls back to the silent clock when decoding fails", async () => {
    const { player, decodes } = harness();
    player.setSection("a", DATA, LOOP);
    player.play();
    decodes[0]?.reject(new Error("bad mp3"));
    await settle();
    expect(player.getSnapshot()).toMatchObject({ playing: true, loading: false, notice: "decodeFailed" });
    expect(player.getSnapshot().clock?.playing).toBe(true);
  });

  it("falls back to the silent clock when no audio context can be created", () => {
    const { player } = harness({
      createContext: () => {
        throw new Error("no WebAudio");
      },
    });
    player.setSection("a", DATA, LOOP);
    player.play();
    expect(player.getSnapshot()).toMatchObject({ notice: "decodeFailed", loading: false });
    expect(player.getSnapshot().clock?.playing).toBe(true);
  });

  it("plays the audio at the playback rate, rebuilding the playing loop when the rate changes", async () => {
    const { player, contexts, decodes } = harness();
    player.setRate(1.5);
    player.setSection("a", DATA, LOOP);
    player.play();
    decodes[0]?.resolve(BUFFER);
    await settle();
    const ctx = onlyContext(contexts);
    expect(ctx.sources.map((s) => s.playbackRate.value)).toEqual([1.5, 1.5]);
    const first = player.getSnapshot().clock;

    player.setRate(0.75);
    const second = player.getSnapshot().clock;
    expect(second).not.toBe(first);
    expect(second?.playing).toBe(true);
    expect(ctx.sources.at(-1)?.playbackRate.value).toBe(0.75);

    player.setRate(0.75);
    expect(player.getSnapshot().clock).toBe(second);
  });

  it("runs the silent clock at the playback rate", () => {
    const { player, now } = harness();
    player.setRate(2);
    player.setSection("a", { kind: "missing" }, LOOP);
    player.play();
    now.ms = 250;
    expect(player.getSnapshot().clock?.nowMs()).toBe(1500);
  });

  it("applies the offset to the audio clock without rebuilding it", async () => {
    const { player, decodes } = harness();
    player.setOffsetMs(20);
    player.setSection("a", DATA, LOOP);
    player.play();
    decodes[0]?.resolve(BUFFER);
    await settle();
    const { clock } = player.getSnapshot();
    expect(clock?.nowMs()).toBe(1020);
    player.setOffsetMs(-30);
    expect(player.getSnapshot().clock).toBe(clock);
    expect(clock?.nowMs()).toBe(970);
  });

  it("toggles between playing and paused and notifies subscribers", () => {
    const { player } = harness();
    let notified = 0;
    const unsubscribe = player.subscribe(() => {
      notified++;
    });
    player.setSection("a", { kind: "missing" }, LOOP);
    player.toggle();
    expect(player.getSnapshot().playing).toBe(true);
    player.toggle();
    expect(player.getSnapshot().playing).toBe(false);
    expect(player.getSnapshot().clock?.playing).toBe(false);
    expect(notified).toBeGreaterThanOrEqual(3);
    unsubscribe();
  });

  it("closes the context on release and starts over on the next play", async () => {
    const { player, contexts, decodes } = harness();
    player.setSection("a", DATA, LOOP);
    player.play();
    decodes[0]?.resolve(BUFFER);
    await settle();
    const clock = player.getSnapshot().clock;

    player.release();
    expect(contexts[0]?.closed).toBe(1);
    expect(clock?.playing).toBe(false);
    expect(player.getSnapshot()).toEqual({ clock: null, playing: false, loading: false, notice: null });

    player.play();
    expect(contexts).toHaveLength(2);
    expect(decodes).toHaveLength(2);
  });
});

describe("skipWithin", () => {
  it("steps forward and back inside the loop, wrapping at either end as the loop does", () => {
    expect(skipWithin(1500, 1000, LOOP)).toBe(2500);
    expect(skipWithin(2500, 1000, LOOP)).toBe(1500);
    expect(skipWithin(1500, -1000, LOOP)).toBe(2500);
    expect(skipWithin(1000, -2000, LOOP)).toBe(1000);
    expect(skipWithin(2999, 1, LOOP)).toBe(1000);
  });

  it("brings a position outside the loop back into it", () => {
    expect(skipWithin(400, 0, LOOP)).toBe(1000);
    expect(skipWithin(4000, 0, LOOP)).toBe(1000);
  });
});

describe("SectionPlayer seeking", () => {
  it("reads the loop start before anything plays, and 0 without a section", () => {
    const { player } = harness();
    expect(player.positionMs()).toBe(0);
    player.setSection("a", DATA, LOOP);
    expect(player.positionMs()).toBe(1000);
  });

  it("keeps a seek made before the first play without opening an audio context, then plays from it", () => {
    const { player, contexts, now } = harness();
    player.setSection("a", { kind: "missing" }, LOOP);
    player.seek(2200);
    expect(contexts).toEqual([]);
    expect(player.getSnapshot().clock).toBeNull();
    expect(player.positionMs()).toBe(2200);

    player.play();
    now.ms = 100;
    expect(player.getSnapshot().clock?.nowMs()).toBe(2300);
    expect(player.positionMs()).toBe(2300);
  });

  it("seeks the playing clock in place: same clock, still playing, nothing rebuilt", () => {
    const { player, now } = harness();
    player.setSection("a", { kind: "missing" }, LOOP);
    player.play();
    const { clock } = player.getSnapshot();
    now.ms = 300;
    player.seek(2500);
    expect(player.getSnapshot().clock).toBe(clock);
    expect(clock?.playing).toBe(true);
    expect(player.positionMs()).toBe(2500);
  });

  it("holds a seek made while the audio decodes and applies it to the clock that follows", async () => {
    const { player, decodes, contexts } = harness();
    player.setSection("a", DATA, LOOP);
    player.play();
    player.seek(2000);
    expect(player.positionMs()).toBe(2000);
    decodes[0]?.resolve(BUFFER);
    await settle();
    const ctx = onlyContext(contexts);
    expect(ctx.sources[0]?.starts[0]?.offset).toBe(2);
    expect(player.positionMs()).toBe(2000);
  });

  it("drops a pending seek when the section changes, since it belonged to the old loop", () => {
    const { player } = harness();
    player.setSection("a", DATA, LOOP);
    player.seek(2000);
    player.setSection("a", DATA, { startMs: 5000, endMs: 7000 });
    expect(player.positionMs()).toBe(5000);
  });

  it("skips relative to where it is, wrapping inside the loop", () => {
    const { player, now } = harness();
    player.setSection("a", { kind: "missing" }, LOOP);
    player.skip(-500);
    expect(player.positionMs()).toBe(2500);
    player.play();
    now.ms = 200;
    player.skip(2000);
    expect(player.positionMs()).toBe(2700);
  });

  it("reports where the preview should stand: nothing before a seek or a clock, then the seek, then the clock", () => {
    const { player, now } = harness();
    player.setSection("a", { kind: "missing" }, LOOP);
    expect(player.pausedAtMs()).toBeNull();
    player.seek(2200);
    expect(player.pausedAtMs()).toBe(2200);
    player.play();
    now.ms = 300;
    player.pause();
    expect(player.pausedAtMs()).toBe(2500);
    player.seek(1500);
    expect(player.pausedAtMs()).toBe(1500);
  });

  it("holds a seek to the loop end made before the first play just before the end, as the clock does", () => {
    const { player } = harness();
    player.setSection("a", { kind: "missing" }, LOOP);
    player.seek(LOOP.endMs);
    expect(player.positionMs()).toBe(LOOP.endMs - SEEK_END_GUARD_MS);
    player.play();
    expect(player.positionMs()).toBe(LOOP.endMs - SEEK_END_GUARD_MS);
  });

  it("ignores a seek with no section", () => {
    const { player } = harness();
    player.seek(1234);
    player.skip(1000);
    expect(player.positionMs()).toBe(0);
  });
});
