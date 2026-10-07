// Structural slices of the WebAudio API, so tests inject fakes and this module never touches a global AudioContext.

export interface AudioBufferLike {
  /** Seconds. */
  readonly duration: number;
}

export interface AudioBufferSourceNodeLike {
  buffer: AudioBufferLike | null;
  onended: ((ev: Event) => unknown) | null;
  /** Optional so minimal fakes need not model it; a rate of 1 never touches it. */
  readonly playbackRate?: { value: number };
  connect(destination: object): unknown;
  disconnect(): void;
  /** Seconds, all three. */
  start(when?: number, offset?: number, duration?: number): void;
  stop(): void;
}

export interface AudioParamLike {
  setValueAtTime(value: number, startTime: number): unknown;
  linearRampToValueAtTime(value: number, endTime: number): unknown;
}

export interface GainNodeLike {
  readonly gain: AudioParamLike;
  connect(destination: object): unknown;
  disconnect(): void;
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
  createGain(): GainNodeLike;
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

export interface SeekableClock extends Clock {
  /**
   * Moves to a chart time inside the loop, playing or paused; `nowMs()` reads it right after. Outside the loop it
   * clamps; a target at or past the end holds `SEEK_END_GUARD_MS` before it.
   */
  seekMs(chartMs: number): void;
}

export interface AudioLoopClock extends SeekableClock {
  /** Wall-clock ms whatever the playback rate, as the viewer's output latency is. */
  setOffsetMs(offsetMs: number): void;
}

/** How one loop iteration hands over to the next. */
export interface LoopSpliceParams {
  /** Fade at both edges of every iteration: a hard cut mid-phrase clicks. */
  fadeMs: number;
  /** Silence between iterations, so a restart is heard as a restart. The clock holds the loop start meanwhile. */
  gapMs: number;
}

const MS_PER_S = 1000;

/**
 * How far before the loop end an absolute seek at or past it lands. Wrapping instead would send a drag to the seek bar's
 * right edge, or a target within the latency compensation of it, back to the section's start.
 */
export const SEEK_END_GUARD_MS = 1;

/** Where a seek to `chartMs` lands, as ms into the loop. */
function seekElapsed(chartMs: number, loop: LoopSpan): number {
  const len = loop.endMs - loop.startMs;
  return Math.max(0, Math.min(chartMs - loop.startMs, len - SEEK_END_GUARD_MS));
}

/** Where `elapsedMs` of looped playback lands; before the first sample is heard and during the gap it stays at the start. */
export function loopPosition(elapsedMs: number, loop: LoopSpan, gapMs = 0): number {
  const len = loop.endMs - loop.startMs;
  if (elapsedMs <= 0 || len <= 0) {
    return loop.startMs;
  }
  const inPeriod = elapsedMs % (len + Math.max(0, gapMs));
  return inPeriod < len ? loop.startMs + inPeriod : loop.startMs;
}

interface Voice {
  source: AudioBufferSourceNodeLike;
  gain: GainNodeLike;
}

export function createAudioLoopClock(
  ctx: AudioContextLike,
  buffer: AudioBufferLike,
  loop: LoopSpan,
  offsetMs: number,
  splice: LoopSpliceParams,
  rate = 1,
): AudioLoopClock {
  // A one-shot asked to play past the buffer ends early, which would desync the clock from the sound.
  const span: LoopSpan = { startMs: loop.startMs, endMs: Math.min(loop.endMs, buffer.duration * MS_PER_S) };
  if (span.endMs <= span.startMs) {
    // The section lies past the end of the audio: an empty span would freeze the clock at its start.
    return contextSilentLoopClock(ctx, loop, offsetMs, splice.gapMs, rate);
  }
  const lenMs = span.endMs - span.startMs;
  const gapMs = Math.max(0, splice.gapMs);
  // Context (wall) time per iteration: the section plays `rate` times faster, the gap is heard and keeps its length.
  const periodMs = lenMs / rate + gapMs;
  let offset = offsetMs;
  // Scheduled (not heard) chart time into the loop: what was already sent to the device must not be sent again.
  let pausedElapsedMs = 0;
  let playing = false;
  // The iteration playing (or waiting out the gap) first, then the one scheduled ahead of it.
  let voices: Voice[] = [];
  let iteration = 0;
  let startedAtS = 0;
  let disposed = false;

  const latencyMs = (): number => (ctx.outputLatency ?? ctx.baseLatency ?? 0) * MS_PER_S;
  const scheduledWallMs = (): number => (playing ? (ctx.currentTime - startedAtS) * MS_PER_S : pausedElapsedMs / rate);
  // A wall-time gap of g is g × rate in chart time, so the chart-time loop keeps the wall period.
  const chartPosition = (wallMs: number): number => loopPosition(wallMs * rate, span, gapMs * rate);
  const heardMs = (): number => chartPosition(scheduledWallMs() - latencyMs());

  const detach = (voice: Voice): void => {
    voice.source.onended = null;
    voice.source.disconnect();
    voice.gain.disconnect();
  };

  // Each iteration is its own one-shot source, scheduled one iteration ahead on the audio clock: `node.loop` cannot
  // fade or pause between iterations, and timers on the main thread drift.
  const schedule = (whenS: number, fromMs: number, durationMs: number): void => {
    // A source node plays once per spec, so every iteration and every resume needs a fresh one.
    const source = ctx.createBufferSource();
    const gain = ctx.createGain();
    source.buffer = buffer;
    if (rate !== 1 && source.playbackRate !== undefined) {
      source.playbackRate.value = rate;
    }
    source.connect(gain);
    gain.connect(ctx.destination);
    const wallMs = durationMs / rate;
    const durationS = wallMs / MS_PER_S;
    const fadeS = Math.min(Math.max(0, splice.fadeMs), wallMs / 2) / MS_PER_S;
    gain.gain.setValueAtTime(0, whenS);
    gain.gain.linearRampToValueAtTime(1, whenS + fadeS);
    gain.gain.setValueAtTime(1, whenS + durationS - fadeS);
    gain.gain.linearRampToValueAtTime(0, whenS + durationS);
    // Per spec the one-shot's duration is buffer content, not context time.
    source.start(whenS, fromMs / MS_PER_S, durationMs / MS_PER_S);
    const voice: Voice = { source, gain };
    source.onended = () => {
      if (voices[0] !== voice) {
        return;
      }
      detach(voice);
      voices = voices.slice(1);
      scheduleNext();
    };
    voices.push(voice);
  };
  const scheduleNext = (): void => {
    // After a main-thread stall a pass due in the past would start now, off the clock's grid and over the next one.
    const dueIteration = Math.ceil(((ctx.currentTime - startedAtS) * MS_PER_S) / periodMs);
    iteration = Math.max(iteration + 1, dueIteration);
    schedule(startedAtS + (iteration * periodMs) / MS_PER_S, span.startMs, lenMs);
  };

  const stop = (): void => {
    if (!playing) {
      return;
    }
    pausedElapsedMs = chartPosition(scheduledWallMs()) - span.startMs;
    playing = false;
    for (const voice of voices) {
      detach(voice);
      voice.source.stop();
    }
    voices = [];
  };

  const start = (): void => {
    if (disposed || playing) {
      return;
    }
    if (ctx.state === "suspended") {
      void ctx.resume();
    }
    const nowS = ctx.currentTime;
    startedAtS = nowS - pausedElapsedMs / rate / MS_PER_S;
    iteration = 0;
    playing = true;
    schedule(nowS, span.startMs + pausedElapsedMs, lenMs - pausedElapsedMs);
    scheduleNext();
  };

  return {
    get playing() {
      return playing;
    },
    // The offset is a latency correction in wall ms; at rate r it covers r times as much chart.
    nowMs: () => heardMs() + offset * rate,
    play: start,
    pause: stop,
    seekMs(chartMs) {
      if (disposed) {
        return;
      }
      const resume = playing;
      stop();
      // The sound reaches the viewer a latency later and is drawn shifted by the offset; scheduling ahead of both makes
      // the drawn position land on the target.
      pausedElapsedMs = seekElapsed(chartMs + (latencyMs() - offset) * rate, span);
      if (resume) {
        start();
      }
    },
    dispose() {
      stop();
      disposed = true;
    },
    setOffsetMs(next) {
      offset = next;
    },
  };
}

function contextSilentLoopClock(
  ctx: AudioContextLike,
  loop: LoopSpan,
  offsetMs: number,
  gapMs: number,
  rate: number,
): AudioLoopClock {
  const silent = createSilentLoopClock(loop, () => ctx.currentTime * MS_PER_S, gapMs, rate);
  let offset = offsetMs;
  return {
    get playing() {
      return silent.playing;
    },
    nowMs: () => silent.nowMs() + offset * rate,
    play() {
      // A suspended context's currentTime stands still.
      if (ctx.state === "suspended") {
        void ctx.resume();
      }
      silent.play();
    },
    pause: () => {
      silent.pause();
    },
    seekMs: (chartMs) => {
      silent.seekMs(chartMs - offset * rate);
    },
    dispose: () => {
      silent.dispose();
    },
    setOffsetMs(next) {
      offset = next;
    },
  };
}

/** Same loop semantics, gap included, on a wall clock, for charts without usable audio. */
export function createSilentLoopClock(
  loop: LoopSpan,
  nowFn: () => number = () => performance.now(),
  gapMs = 0,
  rate = 1,
): SeekableClock {
  let positionMs = loop.startMs;
  let startedAtMs: number | null = null;
  let disposed = false;

  const currentMs = (): number =>
    startedAtMs === null ? positionMs : loopPosition((nowFn() - startedAtMs) * rate, loop, gapMs * rate);
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
      startedAtMs = nowFn() - (positionMs - loop.startMs) / rate;
    },
    pause: stop,
    seekMs(chartMs) {
      if (disposed) {
        return;
      }
      const target = loop.startMs + seekElapsed(chartMs, loop);
      if (startedAtMs === null) {
        positionMs = target;
      } else {
        startedAtMs = nowFn() - (target - loop.startMs) / rate;
      }
    },
    dispose() {
      stop();
      disposed = true;
    },
  };
}
