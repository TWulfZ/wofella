import { StrictMode } from "react";
import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AnchorDto, ChartWindowDto, LabelStatsDto, LabelWindowDto, SampleRequestDto } from "@/ipc/bindings";
import { mockCommands } from "@/ipc/mocks";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { LABEL_SCREEN_PARAMS, LabelScreen, type LabelScreenParams } from "./LabelScreen";
import type { ClosableAudioContext } from "./sectionPlayer";

// Live audio nodes across the whole label screen: React effects (StrictMode's double run included), reshapes and
// window changes must never leave more than the playing iteration plus the one scheduled ahead.

const ANCHOR_A: AnchorDto = { md5: "a".repeat(32), t0Ms: 1000, t1Ms: 5000, cols: [1, 2, 3, 4, 5, 6, 7] };
const ANCHOR_B: AnchorDto = { md5: "b".repeat(32), t0Ms: 20_000, t1Ms: 24_000, cols: [1, 2, 3, 4, 5, 6, 7] };

function labelWindow(anchor: AnchorDto, title: string): LabelWindowDto {
  return { anchor, title, artist: "x", version: "v", level: "dan:7", stratum: "s", played: true };
}

const STATS: LabelStatsDto = {
  total: 0,
  noPattern: 0,
  mixed: 0,
  unsure: 0,
  thumbLeft: 0,
  thumbRight: 0,
  perPattern: [],
  perAxis: [],
  perStratum: [],
};

function chartWindow(md5: string, fromMs: number, toMs: number): ChartWindowDto {
  return {
    md5,
    keymode: 7,
    fromMs,
    toMs,
    notes: [],
    timing: [],
    layout: { id: "k7", columns: Array.from({ length: 7 }, () => ({ hand: "left" as const, finger: "index" as const })) },
    chartSpan: { firstMs: 0, endMs: 90_000 },
    audioFilename: "a.mp3",
  };
}

interface FakeSource {
  starts: { when: number | undefined; offset: number | undefined; duration: number | undefined }[];
  live: boolean;
  end(): void;
}

class Audio {
  sources: FakeSource[] = [];
  contextsAlive = 0;
  maxContexts = 0;
  maxLive = 0;
  decodeGate: (() => void) | null = null;
  gateDecodes = false;

  get live(): number {
    return this.sources.filter((s) => s.live).length;
  }

  createContext = (): ClosableAudioContext => {
    this.contextsAlive++;
    this.maxContexts = Math.max(this.maxContexts, this.contextsAlive);
    const node = () => ({ connect: () => undefined, disconnect: () => undefined });
    const ctx: ClosableAudioContext = {
      currentTime: 0,
      state: "running",
      destination: {},
      createBufferSource: () => {
        let onended: ((ev: Event) => unknown) | null = null;
        const fake: FakeSource = {
          starts: [],
          live: false,
          end: () => {
            fake.live = false;
            onended?.(new Event("ended"));
          },
        };
        this.sources.push(fake);
        return {
          ...node(),
          buffer: null,
          get onended() {
            return onended;
          },
          set onended(handler) {
            onended = handler;
          },
          start: (when?: number, offset?: number, duration?: number) => {
            fake.starts.push({ when, offset, duration });
            fake.live = true;
            this.maxLive = Math.max(this.maxLive, this.live);
          },
          stop: () => {
            fake.live = false;
          },
        };
      },
      createGain: () => ({
        ...node(),
        gain: { setValueAtTime: () => undefined, linearRampToValueAtTime: () => undefined },
      }),
      resume: async () => Promise.resolve(),
      decodeAudioData: async () => {
        if (!this.gateDecodes) {
          return Promise.resolve({ duration: 120 });
        }
        return new Promise((resolve) => {
          this.decodeGate = () => {
            resolve({ duration: 120 });
          };
        });
      },
      close: async () => {
        this.contextsAlive--;
        return Promise.resolve();
      },
    };
    return ctx;
  };
}

class NoopResizeObserver {
  observe(): void {
    // jsdom does no layout.
  }
  unobserve(): void {
    // See observe.
  }
  disconnect(): void {
    // See observe.
  }
}

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", NoopResizeObserver);
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    fillStyle: "#000",
    globalAlpha: 1,
    fillRect: () => undefined,
    setTransform: () => undefined,
  } as unknown as RenderingContext);
  return () => {
    vi.unstubAllGlobals();
  };
});

function render(audio: Audio, strict: boolean, params: LabelScreenParams = LABEL_SCREEN_PARAMS) {
  const windows = [labelWindow(ANCHOR_A, "Alpha"), labelWindow(ANCHOR_B, "Beta")];
  mockCommands({
    labelTaxonomy: () => [],
    labelPatternExamples: () => [],
    labelStats: () => STATS,
    labelSample: (args) => windows[(args["req"] as SampleRequestDto).round] ?? null,
    chartWindow: (args) => chartWindow(String(args["md5"]), Number(args["fromMs"]), Number(args["toMs"])),
    chartAudio: () => ({ mime: "audio/mpeg", base64: "SUQz" }),
    labelSubmit: () => ({ id: "E" }),
    labelUndo: () => null,
    skinList: () => ({ skins: [], current: null, maniaSpeed: null, maniaSpeedBpmScale: null }),
    labelReshape: (args) => {
      const anchor = args["anchor"] as AnchorDto;
      return { ...anchor, t0Ms: anchor.t0Ms + 2000, t1Ms: anchor.t1Ms + 2000 };
    },
  });
  const ui = <LabelScreen keymode={7} seed="42" createAudioContext={audio.createContext} params={params} />;
  return renderWithRouter(strict ? <StrictMode>{ui}</StrictMode> : ui, { path: "/label" });
}

function oldestLive(audio: Audio): FakeSource {
  const source = audio.sources.find((s) => s.live);
  if (source === undefined) {
    throw new Error("no live source");
  }
  return source;
}

describe("LabelScreen section audio", () => {
  for (const strict of [false, true]) {
    it(`keeps one playing and one scheduled source through toggles, iterations, reshape and skip (strict=${strict})`, async () => {
      const audio = new Audio();
      render(audio, strict);
      await userEvent.click(await screen.findByRole("button", { name: "Play" }));
      await waitFor(() => {
        expect(audio.live).toBe(2);
      });

      for (let i = 0; i < 10; i++) {
        oldestLive(audio).end();
        expect(audio.live).toBe(2);
      }

      for (let i = 0; i < 20; i++) {
        fireEvent.keyDown(document.body, { key: " " });
      }
      expect(audio.live).toBe(2);
      fireEvent.keyDown(document.body, { key: " " });
      expect(audio.live).toBe(0);
      fireEvent.keyDown(document.body, { key: " " });

      await userEvent.click(screen.getByRole("button", { name: "Shift window later" }));
      await waitFor(() => {
        expect(audio.sources.some((s) => s.live && s.starts[0]?.offset === 2)).toBe(true);
      });
      expect(audio.live).toBe(2);

      await userEvent.click(screen.getByRole("button", { name: "Skip" }));
      await screen.findByRole("heading", { name: "Beta" });
      fireEvent.keyDown(document.body, { key: " " });
      await waitFor(() => {
        expect(audio.live).toBe(2);
      });
      expect(audio.maxLive).toBe(2);
      expect(audio.maxContexts).toBe(1);
    });
  }

  it("starts nothing when the decode settles after unmount (strict)", async () => {
    const audio = new Audio();
    audio.gateDecodes = true;
    const { view } = render(audio, true);
    await userEvent.click(await screen.findByRole("button", { name: "Play" }));
    await waitFor(() => {
      expect(audio.decodeGate).not.toBeNull();
    });
    view.unmount();
    await act(async () => {
      audio.decodeGate?.();
      await Promise.resolve();
    });
    expect(audio.sources).toEqual([]);
    expect(audio.contextsAlive).toBe(0);
  });

  it("splices the loop with the label screen params", async () => {
    const audio = new Audio();
    render(audio, false, { ...LABEL_SCREEN_PARAMS, loopSplice: { fadeMs: 30, gapMs: 500 } });
    await userEvent.click(await screen.findByRole("button", { name: "Play" }));
    await waitFor(() => {
      expect(audio.live).toBe(2);
    });
    // Anchor A loops 0–5250 ms: preroll 1000 before t0 = 1000 clamps at 0, postroll 250 after t1 = 5000.
    expect(audio.sources.map((s) => s.starts)).toEqual([
      [{ when: 0, offset: 0, duration: 5.25 }],
      [{ when: 5.75, offset: 0, duration: 5.25 }],
    ]);
  });
});
