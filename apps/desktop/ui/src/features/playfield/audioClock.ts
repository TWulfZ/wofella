// Structural slices of the WebAudio API, so tests inject fakes and this module never touches a global AudioContext.

export interface AudioBufferLike {
  /** Seconds. */
  readonly duration: number;
}

export interface AudioBufferSourceNodeLike {
  buffer: AudioBufferLike | null;
  loop: boolean;
  /** Seconds. */
  loopStart: number;
  /** Seconds. */
  loopEnd: number;
  connect(destination: object): unknown;
  disconnect(): void;
  start(when?: number, offset?: number): void;
  stop(): void;
}

export interface AudioContextLike {
  /** Seconds. */
  readonly currentTime: number;
  /** Seconds; missing on some WebKit builds. */
  readonly outputLatency?: number;
  /** Seconds. */
  readonly baseLatency?: number;
  readonly destination: object;
  readonly state: string;
  resume(): Promise<void>;
  createBufferSource(): AudioBufferSourceNodeLike;
  decodeAudioData(data: ArrayBuffer): Promise<AudioBufferLike>;
}

export interface LoopSpan {
  startMs: number;
  endMs: number;
}

export interface Clock {
  /** Chart time, in ms. */
  nowMs(): number;
  readonly playing: boolean;
  play(): void;
  pause(): void;
  /** Final: a disposed clock never plays again. */
  dispose(): void;
}

export interface AudioLoopClock extends Clock {
  setOffsetMs(offsetMs: number): void;
}

const MS_PER_S = 1000;

/** Where `elapsedMs` of looped playback lands; before the first sample is heard it stays at the start. */
export function loopPosition(elapsedMs: number, loop: LoopSpan): number {
  const len = loop.endMs - loop.startMs;
  if (elapsedMs <= 0 || len <= 0) {
    return loop.startMs;
  }
  return loop.startMs + (elapsedMs % len);
}

export function createAudioLoopClock(
  ctx: AudioContextLike,
  buffer: AudioBufferLike,
  loop: LoopSpan,
  offsetMs: number,
): AudioLoopClock {
  // WebAudio silently plays the whole buffer when loopEnd lies past it, which would desync the clock from the sound.
  const span: LoopSpan = { startMs: loop.startMs, endMs: Math.min(loop.endMs, buffer.duration * MS_PER_S) };
  let offset = offsetMs;
  let positionMs = span.startMs;
  let source: AudioBufferSourceNodeLike | null = null;
  let startedAtS = 0;
  let disposed = false;

  const latencyMs = (): number => (ctx.outputLatency ?? ctx.baseLatency ?? 0) * MS_PER_S;
  const heardMs = (): number =>
    source === null ? positionMs : loopPosition((ctx.currentTime - startedAtS) * MS_PER_S - latencyMs(), span);

  const stop = (): void => {
    if (source === null) {
      return;
    }
    positionMs = heardMs();
    source.stop();
    source.disconnect();
    source = null;
  };

  return {
    get playing() {
      return source !== null;
    },
    nowMs: () => heardMs() + offset,
    play() {
      if (disposed || source !== null) {
        return;
      }
      if (ctx.state === "suspended") {
        void ctx.resume();
      }
      // A source node plays once per spec, so every resume needs a fresh one.
      const node = ctx.createBufferSource();
      node.buffer = buffer;
      node.loop = true;
      node.loopStart = span.startMs / MS_PER_S;
      node.loopEnd = span.endMs / MS_PER_S;
      node.connect(ctx.destination);
      node.start(0, positionMs / MS_PER_S);
      startedAtS = ctx.currentTime - (positionMs - span.startMs) / MS_PER_S;
      source = node;
    },
    pause: stop,
    dispose() {
      stop();
      disposed = true;
    },
    setOffsetMs(next) {
      offset = next;
    },
  };
}

/** Same loop semantics on a wall clock, for charts without usable audio. */
export function createSilentLoopClock(loop: LoopSpan, nowFn: () => number = () => performance.now()): Clock {
  let positionMs = loop.startMs;
  let startedAtMs: number | null = null;
  let disposed = false;

  const currentMs = (): number => (startedAtMs === null ? positionMs : loopPosition(nowFn() - startedAtMs, loop));
  const stop = (): void => {
    positionMs = currentMs();
    startedAtMs = null;
  };

  return {
    get playing() {
      return startedAtMs !== null;
    },
    nowMs: currentMs,
    play() {
      if (disposed || startedAtMs !== null) {
        return;
      }
      startedAtMs = nowFn() - (positionMs - loop.startMs);
    },
    pause: stop,
    dispose() {
      stop();
      disposed = true;
    },
  };
}
