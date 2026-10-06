import {
  type AudioBufferLike,
  type AudioContextLike,
  type AudioLoopClock,
  type Clock,
  createAudioLoopClock,
  createSilentLoopClock,
  decodeBase64Audio,
  type LoopSpan,
} from "@/features/playfield";

export type AudioInput = { kind: "pending" } | { kind: "data"; base64: string } | { kind: "missing" };

export type AudioNotice = "unavailable" | "decodeFailed" | null;

export interface SectionPlayerSnapshot {
  /** Null before the first play and while the audio is decoding. */
  clock: Clock | null;
  /** What the user asked for; the clock follows once it exists. */
  playing: boolean;
  loading: boolean;
  notice: AudioNotice;
}

export type ClosableAudioContext = AudioContextLike & { close?: () => Promise<void> };

export interface SectionPlayerDeps {
  createContext: () => ClosableAudioContext;
  decode?: (ctx: AudioContextLike, base64: string) => Promise<AudioBufferLike>;
  /** Wall clock for the silent fallback, in ms. */
  now?: () => number;
}

const IDLE: SectionPlayerSnapshot = { clock: null, playing: false, loading: false, notice: null };

/**
 * Owns the one AudioContext of the label screen and the loop clock of the current section. A plain object with a
 * subscribe/getSnapshot pair, so React reads it through useSyncExternalStore and tests drive it without a DOM.
 */
export class SectionPlayer {
  private readonly deps: SectionPlayerDeps;
  private md5: string | null = null;
  private audio: AudioInput = { kind: "pending" };
  private loop: LoopSpan | null = null;
  private offsetMs = 0;
  private ctx: ClosableAudioContext | null = null;
  private ctxFailed = false;
  // One chart's buffer only: a decoded song is tens of MB of PCM, and reshapes keep the same md5.
  private decoded: { md5: string; buffer: AudioBufferLike | null } | null = null;
  private decoding: string | null = null;
  private clock: Clock | null = null;
  private audioClock: AudioLoopClock | null = null;
  private started = false;
  private wantPlaying = false;
  private snapshot: SectionPlayerSnapshot = IDLE;
  private readonly listeners = new Set<() => void>();

  constructor(deps: SectionPlayerDeps) {
    this.deps = deps;
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  getSnapshot = (): SectionPlayerSnapshot => this.snapshot;

  setSection(md5: string, audio: AudioInput, loop: LoopSpan): void {
    const sameAudio =
      this.audio.kind === audio.kind && (audio.kind !== "data" || (this.audio.kind === "data" && this.audio.base64 === audio.base64));
    if (
      this.md5 === md5 &&
      sameAudio &&
      this.loop !== null &&
      this.loop.startMs === loop.startMs &&
      this.loop.endMs === loop.endMs
    ) {
      return;
    }
    this.md5 = md5;
    this.audio = audio;
    this.loop = loop;
    this.rebuild();
  }

  setOffsetMs(offsetMs: number): void {
    this.offsetMs = offsetMs;
    this.audioClock?.setOffsetMs(offsetMs);
  }

  play(): void {
    this.wantPlaying = true;
    if (!this.started) {
      this.started = true;
      this.ensureContext();
      this.rebuild();
      return;
    }
    if (this.clock === null) {
      this.rebuild();
      return;
    }
    this.clock.play();
    this.emit();
  }

  pause(): void {
    this.wantPlaying = false;
    this.clock?.pause();
    this.emit();
  }

  toggle(): void {
    if (this.wantPlaying) {
      this.pause();
    } else {
      this.play();
    }
  }

  /** Frees the audio; the section stays known, so a later play starts over with a new context. */
  release(): void {
    this.dropClock();
    void this.ctx?.close?.();
    this.ctx = null;
    this.ctxFailed = false;
    this.decoded = null;
    this.decoding = null;
    this.started = false;
    this.wantPlaying = false;
    this.emit();
  }

  private ensureContext(): void {
    if (this.ctx !== null || this.ctxFailed) {
      return;
    }
    try {
      this.ctx = this.deps.createContext();
    } catch {
      this.ctxFailed = true;
    }
  }

  private dropClock(): void {
    this.clock?.dispose();
    this.clock = null;
    this.audioClock = null;
  }

  /** The decode outcome for the current chart: a buffer, `null` when decoding failed, `undefined` before it ends. */
  private currentBuffer(): AudioBufferLike | null | undefined {
    return this.decoded?.md5 === this.md5 ? this.decoded.buffer : undefined;
  }

  private rebuild(): void {
    this.dropClock();
    const { md5, loop, ctx } = this;
    if (this.started && md5 !== null && loop !== null) {
      const buffer = this.currentBuffer();
      const silent = this.audio.kind === "missing" || this.ctxFailed || buffer === null;
      if (silent) {
        this.clock = createSilentLoopClock(loop, this.deps.now);
      } else if (buffer !== undefined && ctx !== null) {
        this.audioClock = createAudioLoopClock(ctx, buffer, loop, this.offsetMs);
        this.clock = this.audioClock;
      } else if (this.audio.kind === "data" && ctx !== null) {
        this.startDecode(ctx, md5, this.audio.base64);
      }
      if (this.clock !== null && this.wantPlaying) {
        this.clock.play();
      }
    }
    this.emit();
  }

  private startDecode(ctx: AudioContextLike, md5: string, base64: string): void {
    if (this.decoding === md5) {
      return;
    }
    this.decoding = md5;
    const decode = this.deps.decode ?? decodeBase64Audio;
    const settle = (buffer: AudioBufferLike | null): void => {
      // A newer chart or a release took over while this one decoded.
      if (this.decoding !== md5 || this.ctx !== ctx) {
        return;
      }
      this.decoding = null;
      this.decoded = { md5, buffer };
      if (this.md5 === md5) {
        this.rebuild();
      }
    };
    decode(ctx, base64).then(settle, () => {
      settle(null);
    });
  }

  private notice(): AudioNotice {
    if (this.audio.kind === "missing") {
      return "unavailable";
    }
    if (this.started && (this.ctxFailed || this.currentBuffer() === null)) {
      return "decodeFailed";
    }
    return null;
  }

  private emit(): void {
    const next: SectionPlayerSnapshot = {
      clock: this.clock,
      playing: this.wantPlaying,
      loading: this.started && this.wantPlaying && this.clock === null,
      notice: this.notice(),
    };
    const prev = this.snapshot;
    if (
      prev.clock === next.clock &&
      prev.playing === next.playing &&
      prev.loading === next.loading &&
      prev.notice === next.notice
    ) {
      return;
    }
    this.snapshot = next;
    for (const listener of this.listeners) {
      listener();
    }
  }
}
