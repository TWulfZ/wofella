import { useQuery } from "@tanstack/react-query";
import {
  type MouseEvent,
  useEffect,
  useEffectEvent,
  useReducer,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { useTranslation } from "react-i18next";
import { type ChartWindow, clampOsuSpeed, DEFAULT_STAGE_PARAMS, Playfield, useLoadedSkin } from "@/features/playfield";
import type { PatternDefDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { AnswerBar, type AnswerFeedback, type AnswerMode } from "./components/AnswerBar";
import { ChartHeader } from "./components/ChartHeader";
import { PanelResizer, usePanelWidth } from "./components/PanelResizer";
import { PatternGrid } from "./components/PatternGrid";
import { SessionFooter } from "./components/SessionFooter";
import { type ToolbarAction, type ToolbarState, SessionToolbar } from "./components/SessionToolbar";
import { SkinPicker } from "./components/SkinPicker";
import { Transport } from "./components/Transport";
import { formatClock } from "./format";
import {
  hasStoredOsuSpeed,
  readOffsetMs,
  readScrollPrefs,
  readSkinChoice,
  readZoom,
  scrollFromPrefs,
  type ScrollPrefs,
  writeFit,
  writeOffsetMs,
  writeOsuSpeed,
  writePxPerMs,
  writeScrollKind,
  writeSkinChoice,
  writeZoom,
} from "./prefs";
import {
  chartAudioQuery,
  chartWindowQuery,
  labelPatternExamplesQuery,
  labelStatsQuery,
  labelTaxonomyQuery,
  skinListQuery,
  useLabelMutations,
  useSkinFile,
} from "./queries";
import { type AudioInput, type ClosableAudioContext, type LoopSpliceParams, SectionPlayer } from "./sectionPlayer";
import {
  canSave,
  canUndo,
  currentEntry,
  type HistoryEntry,
  initialSession,
  isSampling,
  nowPlayingRequest,
  randomRequest,
  sampleExclusion,
  sessionReducer,
  submitPayload,
  undoTarget,
} from "./session";
import { selectedSkinFolder, skinOptions } from "./skins";
import type { Anchor, WindowOp } from "./types";

export interface LabelScreenParams {
  /** Audio heard before the window, so its first notes land in context. */
  prerollMs: number;
  postrollMs: number;
  /** px/ms mode speed until one is chosen. */
  defaultPxPerMs: number;
  /** osu! speed until one is chosen, when neither the caller nor the cfg (ManiaSpeed) gives one. */
  defaultOsuSpeed: number;
  loopSplice: LoopSpliceParams;
}

export const LABEL_SCREEN_PARAMS: LabelScreenParams = {
  prerollMs: 1000,
  postrollMs: 250,
  defaultPxPerMs: 1,
  defaultOsuSpeed: 20,
  loopSplice: { fadeMs: 30, gapMs: 150 },
};

// osu!mania's in-game bindings: F3 slower, F4 faster.
const OSU_SPEED_KEYS: Readonly<Record<string, number>> = { F3: -1, F4: 1 };

const MS_PER_SECOND = 1000;

/** Room kept between a focused card and the answer bar pinned over the panel's bottom edge. */
const ANSWER_BAR_CLEARANCE_PX = 8;

export interface LabelScreenProps {
  keymode: number;
  /** Replays a session; by default each session draws one from the clock, as `wolluf label` does. */
  seed?: string;
  createAudioContext?: () => ClosableAudioContext;
  params?: LabelScreenParams;
  /** Used until a speed is chosen here, ahead of the cfg ManiaSpeed. */
  defaultOsuSpeed?: number;
}

function newSeed(): string {
  return String(Date.now());
}

export function LabelScreen(props: LabelScreenProps) {
  const { keymode, seed, createAudioContext, params = LABEL_SCREEN_PARAMS } = props;
  const defaultOsuSpeed = props.defaultOsuSpeed ?? null;
  const [session, setSession] = useState(() => ({ id: 0, seed: seed ?? newSeed() }));
  return (
    <LabelSession
      key={session.id}
      keymode={keymode}
      seed={session.seed}
      createAudioContext={createAudioContext ?? (() => new AudioContext())}
      params={params}
      defaultOsuSpeed={defaultOsuSpeed}
      onRestart={() => {
        setSession((s) => ({ id: s.id + 1, seed: newSeed() }));
      }}
    />
  );
}

interface LabelSessionProps {
  keymode: number;
  seed: string;
  createAudioContext: () => ClosableAudioContext;
  params: LabelScreenParams;
  defaultOsuSpeed: number | null;
  onRestart: () => void;
}

function useSectionPlayer(createContext: () => ClosableAudioContext, splice: LoopSpliceParams): SectionPlayer {
  const [player] = useState(() => new SectionPlayer({ createContext, splice }));
  useEffect(
    () => () => {
      player.release();
    },
    [player],
  );
  return player;
}

// A failed fetch shows the still placeholders of an id with no example, not loading pulses forever.
const NO_EXAMPLES: ReadonlyMap<string, ChartWindow> = new Map();

const NO_TAXONOMY: readonly PatternDefDto[] = [];

const TEXT_INPUT_TYPES: ReadonlySet<string> = new Set(["text", "search", "number", "email", "url", "tel", "password"]);

function isTextField(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement) {
    return TEXT_INPUT_TYPES.has(target.type);
  }
  return target instanceof HTMLTextAreaElement || (target instanceof HTMLElement && target.isContentEditable);
}

/** Keys these handle themselves (Enter presses a button, Space opens a select). */
const OWN_KEY_CONTROLS = "button, select, a[href], [role='separator'], [role='option'], [role='checkbox']";

/** Hands focus back to the page, where Enter saves and Space plays. */
function releaseFocus(): void {
  const active = document.activeElement;
  if (active instanceof HTMLElement && active !== document.body) {
    active.blur();
  }
}

function LabelSession({ keymode, seed, createAudioContext, params, defaultOsuSpeed, onRestart }: LabelSessionProps) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const [state, dispatch] = useReducer(sessionReducer, seed, initialSession);
  const [feedback, setFeedback] = useState<AnswerFeedback | null>(null);
  const [notice, setNotice] = useState<AnswerFeedback | null>(null);
  const [sampleAttempt, setSampleAttempt] = useState(0);
  const panel = usePanelWidth();
  const [answerBarPx, setAnswerBarPx] = useState<number | null>(null);
  // The entry a save runs against, kept after the save moves past it: an Enter that lands before the next render
  // still holds that entry in its closure, and isPending only flips on that render.
  const actedOn = useRef<HistoryEntry | null>(null);

  const taxonomy = useQuery(labelTaxonomyQuery(keymode));
  const examples = useQuery(labelPatternExamplesQuery(keymode));
  const stats = useQuery(labelStatsQuery());
  const { sample, random, nowPlaying, reshape, submit, undo } = useLabelMutations();
  const entry = currentEntry(state);
  const labelWindow = entry?.window ?? null;
  const anchor: Anchor | null = labelWindow?.anchor ?? null;
  const chart = useQuery(chartWindowQuery(anchor));
  const audio = useQuery(chartAudioQuery(anchor?.md5 ?? null));
  const sampling = isSampling(state);

  const sampledKey = useRef<string | null>(null);
  const sampleWindow = sample.mutate;
  useEffect(() => {
    if (!isSampling(state)) {
      return;
    }
    const key = `${state.round}:${sampleAttempt}`;
    if (sampledKey.current === key) {
      return;
    }
    sampledKey.current = key;
    const { seed: s, round, exclude } = sampleExclusion(state);
    sampleWindow(
      { keymode, seed: s, round, windowMs: null, scale: null, levelMin: null, levelMax: null, exclude },
      {
        onSuccess: (next) => {
          dispatch(
            next === null ? { type: "sessionDone" } : { type: "windowLoaded", window: next, origin: { kind: "plan", round } },
          );
        },
      },
    );
  }, [state, keymode, sampleAttempt, sampleWindow]);

  const player = useSectionPlayer(createAudioContext, params.loopSplice);
  const playback = useSyncExternalStore(player.subscribe, player.getSnapshot);
  const md5 = anchor?.md5 ?? null;
  const audioKind: AudioInput["kind"] = audio.isSuccess ? "data" : audio.isError ? "missing" : "pending";
  const base64 = audio.data?.base64;
  const loopStart = anchor === null ? 0 : Math.max(0, anchor.t0Ms - params.prerollMs);
  const loopEnd = anchor === null ? 0 : anchor.t1Ms + params.postrollMs;
  useEffect(() => {
    if (md5 !== null) {
      return;
    }
    // A finished plan may wait long for Random or Now playing; the audio context is not held meanwhile.
    if (state.done) {
      player.release();
    } else {
      player.pause();
    }
  }, [player, state.done, md5]);
  useEffect(() => {
    if (md5 === null) {
      return;
    }
    const input: AudioInput =
      audioKind === "data" && base64 !== undefined ? { kind: "data", base64 } : { kind: audioKind === "missing" ? "missing" : "pending" };
    player.setSection(md5, input, { startMs: loopStart, endMs: loopEnd });
  }, [player, md5, audioKind, base64, loopStart, loopEnd]);

  const [offsetMs, setOffsetMs] = useState(readOffsetMs);
  useEffect(() => {
    player.setOffsetMs(offsetMs);
  }, [player, offsetMs]);
  const skinList = useQuery(skinListQuery());
  const [skinChoice, setSkinChoice] = useState(readSkinChoice);
  const skinFolder = selectedSkinFolder(skinList.data, skinChoice);
  const skinFile = useSkinFile(skinFolder, keymode, skinList.data);
  const loadedSkin = useLoadedSkin(skinFile.data?.dto ?? null);
  const [skinReloading, setSkinReloading] = useState(false);
  const reloadSkin = async (): Promise<void> => {
    setSkinReloading(true);
    try {
      const { data } = await skinList.refetch();
      // Joins the refetch useSkinFile starts for a changed mtime, so Reload asks the disk once either way.
      if (skinFolder !== null && selectedSkinFolder(data, skinChoice) === skinFolder) {
        await skinFile.refetch({ cancelRefetch: false });
      }
    } finally {
      setSkinReloading(false);
    }
  };

  const [scroll, setScroll] = useState(() =>
    readScrollPrefs({ osuSpeed: params.defaultOsuSpeed, pxPerMs: params.defaultPxPerMs }),
  );
  const [osuSpeedChosen, setOsuSpeedChosen] = useState(hasStoredOsuSpeed);
  const cfgOsuSpeed = skinList.data?.maniaSpeed ?? null;
  // The cfg arrives after the first render, so the default is applied on read instead of seeding the state.
  const effectiveScroll: ScrollPrefs = osuSpeedChosen
    ? scroll
    : { ...scroll, osuSpeed: clampOsuSpeed(defaultOsuSpeed ?? cfgOsuSpeed ?? params.defaultOsuSpeed) };
  const [zoom, setZoom] = useState(readZoom);
  const updateScroll = (patch: Partial<ScrollPrefs>): void => {
    setScroll((current) => ({ ...current, ...patch }));
    if (patch.kind !== undefined) {
      writeScrollKind(patch.kind);
    }
    if (patch.osuSpeed !== undefined) {
      setOsuSpeedChosen(true);
      writeOsuSpeed(patch.osuSpeed);
    }
    if (patch.pxPerMs !== undefined) {
      writePxPerMs(patch.pxPerMs);
    }
    if (patch.fit !== undefined) {
      writeFit(patch.fit);
    }
  };

  const busy = submit.isPending || reshape.isPending || undo.isPending || random.isPending || nowPlaying.isPending;

  const save = async (): Promise<void> => {
    const payload = submitPayload(state);
    if (entry === null || payload === null || actedOn.current === entry) {
      return;
    }
    actedOn.current = entry;
    setFeedback(null);
    let saved = false;
    try {
      const event = await submit.mutateAsync(payload);
      dispatch({ type: "submitted", eventId: event.id, answer: state.answer });
      saved = true;
    } catch (e) {
      setFeedback({ tone: "error", text: errorText(e) });
    } finally {
      if (!saved) {
        actedOn.current = null;
      }
    }
  };

  const runUndo = async (): Promise<void> => {
    const target = undoTarget(state);
    if (!canUndo(state) || target === null) {
      return;
    }
    setFeedback(null);
    try {
      await undo.mutateAsync(target);
      dispatch({ type: "undone", eventId: target });
      setFeedback({ tone: "info", text: t("label.undone") });
    } catch (e) {
      setFeedback({ tone: "error", text: errorText(e) });
    }
  };

  const runReshape = async (op: WindowOp): Promise<void> => {
    if (labelWindow === null) {
      return;
    }
    try {
      const next = await reshape.mutateAsync({ anchor: labelWindow.anchor, op });
      dispatch({ type: "windowReshaped", anchor: next });
    } catch (e) {
      setFeedback({ tone: "error", text: errorText(e) });
    }
  };

  const runRandom = async (): Promise<void> => {
    try {
      const next = await random.mutateAsync({ keymode, windowMs: null, ...randomRequest(state) });
      if (next === null) {
        setNotice({ tone: "info", text: t("label.notice.randomNone") });
      } else {
        dispatch({ type: "windowLoaded", window: next, origin: { kind: "random" } });
      }
    } catch (e) {
      setNotice({ tone: "error", text: errorText(e) });
    }
  };

  const runNowPlaying = async (): Promise<void> => {
    try {
      const found = await nowPlaying.mutateAsync({ keymode, ...nowPlayingRequest(state) });
      if (found === null) {
        setNotice({ tone: "info", text: t("label.notice.nowPlayingNone") });
      } else {
        dispatch({ type: "windowLoaded", window: found.window, origin: { kind: "nowPlaying", source: found.source } });
      }
    } catch (e) {
      setNotice({ tone: "error", text: errorText(e) });
    }
  };

  const onToolbar = (action: ToolbarAction): void => {
    setNotice(null);
    setFeedback(null);
    switch (action) {
      case "previous":
        dispatch({ type: "moved", to: "previous" });
        break;
      case "next":
        dispatch({ type: "moved", to: "next" });
        break;
      case "skip":
        dispatch({ type: "skipped" });
        break;
      case "random":
        void runRandom();
        break;
      case "nowPlaying":
        void runNowPlaying();
        break;
    }
  };

  const clearAnswer = (): void => {
    setFeedback(null);
    dispatch({ type: "answerCleared" });
  };

  const onWindowKey = useEffectEvent((e: KeyboardEvent) => {
    if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey || e.isComposing) {
      return;
    }
    // Work wherever the focus is, as in game.
    const speedStep = OSU_SPEED_KEYS[e.key];
    if (speedStep !== undefined) {
      e.preventDefault();
      if (effectiveScroll.kind === "osu") {
        const osuSpeed = clampOsuSpeed(effectiveScroll.osuSpeed + speedStep * DEFAULT_STAGE_PARAMS.osuSpeedStep);
        updateScroll(effectiveScroll.fit ? { osuSpeed, fit: false } : { osuSpeed });
      }
      return;
    }
    if (isTextField(e.target)) {
      return;
    }
    if (e.key === "Escape") {
      clearAnswer();
      return;
    }
    if (e.target instanceof Element && e.target.closest(OWN_KEY_CONTROLS) !== null) {
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (!e.repeat) {
        void save();
      }
    } else if (e.key === " ") {
      e.preventDefault();
      if (!e.repeat) {
        player.toggle();
      }
    }
  });
  useEffect(() => {
    const listener = (e: KeyboardEvent): void => {
      onWindowKey(e);
    };
    window.addEventListener("keydown", listener);
    return () => {
      window.removeEventListener("keydown", listener);
    };
  }, []);

  // Clicks must not park focus on a button, or the next Enter would press it again instead of saving. Form fields keep
  // their focus so sliders still drag; a click elsewhere also leaves a text field, so Enter saves after a search.
  const keepFocus = (e: MouseEvent): void => {
    if (!(e.target instanceof Element) || e.target.closest("input, textarea, select") !== null) {
      return;
    }
    e.preventDefault();
    if (e.target.closest("[role='dialog']") === null && isTextField(document.activeElement)) {
      releaseFocus();
    }
  };

  const waiting = busy || sampling ? "label.toolbar.busy" : null;
  const planFinished = state.done && entry === null ? "label.toolbar.planFinished" : null;
  const blocked: ToolbarState = {
    previous: waiting ?? (state.cursor === 0 ? "label.toolbar.firstWindow" : null),
    next: waiting ?? planFinished,
    random: waiting,
    nowPlaying: waiting,
    skip: waiting ?? planFinished ?? (entry?.status.kind === "saved" ? "label.toolbar.savedNoSkip" : null),
  };
  const answerMode: AnswerMode =
    entry === null
      ? { kind: "none" }
      : entry.status.kind === "saved"
        ? { kind: "saved", canUndo: canUndo(state) }
        : { kind: "edit", skipped: entry.status.kind === "skipped", canSave: canSave(state) };
  const shownAnswer = entry?.status.kind === "saved" ? entry.status.answer : state.answer;

  const rangeText = anchor === null ? "" : `${formatClock(anchor.t0Ms)}–${formatClock(anchor.t1Ms)}`;
  const durationText =
    anchor === null ? "" : t("label.window.duration", { seconds: ((anchor.t1Ms - anchor.t0Ms) / MS_PER_SECOND).toFixed(1) });
  // Before the list answers the folder is an unconfirmed stored choice; one that is gone falls back silently.
  const skinError = skinList.isError
    ? skinList.error
    : skinFile.isError && skinList.data !== undefined
      ? skinFile.error
      : null;
  const skinNotices = [
    ...(skinError === null ? [] : [errorText(skinError)]),
    ...(loadedSkin.failedSlots.length === 0
      ? []
      : [t("label.skin.decodeFailed", { slots: loadedSkin.failedSlots.join(", ") })]),
    ...(skinList.data?.maniaSpeedBpmScale === true && effectiveScroll.kind === "osu"
      ? [t("label.skin.bpmScaleIgnored")]
      : []),
  ];
  const skin = loadedSkin.skin;
  const skinProps = skin === null ? {} : { skin, hitPosition: skin.hitPosition, columnWidths: skin.columnWidth };
  const audioNotice = audio.isError
    ? errorText(audio.error)
    : playback.notice === "decodeFailed"
      ? t("label.audio.decodeFailed")
      : null;

  return (
    <div
      onMouseDown={keepFocus}
      className={cn(
        "flex h-[calc(100dvh-4rem)] min-h-[32rem] flex-col gap-3 p-4",
        panel.dragging && "cursor-col-resize select-none",
      )}
    >
      <div className="flex min-h-0 flex-1 gap-2">
        <section aria-label={t("label.title")} className="flex min-w-0 flex-1 flex-col gap-3">
          <SessionToolbar blocked={blocked} onAction={onToolbar} />
          {notice !== null && (
            <p
              role={notice.tone === "error" ? "alert" : "status"}
              className={cn(
                "self-start rounded-md px-2.5 py-1 text-xs",
                notice.tone === "error" ? "bg-destructive/10 text-destructive" : "bg-muted/60 text-muted-foreground",
              )}
            >
              {notice.text}
            </p>
          )}
          {state.done && entry === null ? (
            <div role="status" className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
              <p className="text-lg">{t("label.done.exhausted")}</p>
              <p className="text-muted-foreground max-w-sm text-sm">{t("label.done.hint")}</p>
              <Button variant="outline" onClick={onRestart}>
                {t("label.done.newSession")}
              </Button>
            </div>
          ) : labelWindow === null || chart.data === undefined ? (
            <div className="text-muted-foreground flex flex-1 items-center justify-center text-sm">
              {sample.isError ? (
                <div role="alert" className="flex flex-col items-center gap-2">
                  <p className="text-destructive">{errorText(sample.error)}</p>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => {
                      setSampleAttempt((n) => n + 1);
                    }}
                  >
                    {t("common.retry")}
                  </Button>
                </div>
              ) : chart.isError ? (
                <p role="alert" className="text-destructive">
                  {errorText(chart.error)}
                </p>
              ) : (
                <p>{labelWindow === null ? t("label.sampling") : t("label.loadingChart")}</p>
              )}
            </div>
          ) : (
            <>
              <Transport
                playing={playback.playing}
                loading={playback.loading}
                onToggle={() => {
                  player.toggle();
                }}
                range={rangeText}
                duration={durationText}
                offsetMs={offsetMs}
                onOffset={(value) => {
                  setOffsetMs(value);
                  writeOffsetMs(value);
                }}
                scroll={effectiveScroll}
                onScroll={updateScroll}
                zoom={zoom}
                onZoom={(value) => {
                  setZoom(value);
                  writeZoom(value);
                }}
                onSettle={releaseFocus}
                onReshape={(op) => {
                  void runReshape(op);
                }}
                reshapeDisabled={busy || entry?.status.kind === "saved"}
              />
              <SkinPicker
                options={skinOptions(skinList.data, keymode)}
                folder={skinFolder}
                ready={skinList.data !== undefined}
                reloading={skinReloading}
                onChange={(folder) => {
                  setSkinChoice({ folder });
                  writeSkinChoice({ folder });
                  releaseFocus();
                }}
                onReload={() => {
                  void reloadSkin();
                }}
              />
              {skinNotices.map((text) => (
                <p key={text} role="status" className="text-muted-foreground bg-muted/60 self-start rounded-md px-2.5 py-1 text-xs">
                  {text}
                </p>
              ))}
              {audioNotice !== null && (
                <p role="status" className="text-muted-foreground bg-muted/60 self-start rounded-md px-2.5 py-1 text-xs">
                  {audioNotice}
                </p>
              )}
              <div data-testid="playfield" className="flex min-h-0 flex-1 justify-center">
                <Playfield
                  window={chart.data}
                  clock={playback.clock}
                  scroll={scrollFromPrefs(effectiveScroll)}
                  zoom={zoom}
                  {...skinProps}
                  className="h-full w-full"
                />
              </div>
            </>
          )}
        </section>

        <PanelResizer panel={panel} />

        <aside
          data-testid="pattern-panel"
          style={{
            width: panel.width,
            scrollPaddingBottom: answerBarPx === null ? undefined : answerBarPx + ANSWER_BAR_CLEARANCE_PX,
          }}
          className="flex min-h-0 shrink-0 flex-col gap-4 overflow-y-auto pr-2 pl-1 [scrollbar-gutter:stable]"
        >
          {entry !== null && <ChartHeader window={entry.window} origin={entry.origin} />}

          <section aria-label={t("label.patterns")} className="flex flex-col gap-2 pb-4">
            <h3 className="sr-only">{t("label.patterns")}</h3>
            {taxonomy.isError ? (
              <p role="alert" className="text-destructive text-sm">
                {errorText(taxonomy.error)}
              </p>
            ) : taxonomy.data === undefined ? (
              <p className="text-muted-foreground text-sm">{t("common.loading")}</p>
            ) : (
              <PatternGrid
                taxonomy={taxonomy.data}
                examples={examples.isError ? NO_EXAMPLES : examples.data}
                isActive={(pattern) => shownAnswer.patterns.includes(pattern.id)}
                onToggle={(pattern) => {
                  dispatch({ type: "patternToggled", id: pattern.id });
                }}
              />
            )}
          </section>

          <AnswerBar
            taxonomy={taxonomy.data ?? NO_TAXONOMY}
            answer={shownAnswer}
            mode={answerMode}
            busy={busy}
            feedback={feedback}
            onPick={(pattern) => {
              dispatch({ type: "patternAdded", id: pattern.id });
            }}
            onRemove={(id) => {
              dispatch({ type: "patternRemoved", id });
            }}
            onNoPattern={() => {
              dispatch({ type: "noPatternToggled" });
            }}
            onFlag={(toggle) => {
              dispatch({ type: "flagsToggled", toggle });
            }}
            onSave={() => {
              void save();
            }}
            onClear={clearAnswer}
            onUndo={() => {
              void runUndo();
            }}
            onBlockSize={setAnswerBarPx}
          />
        </aside>
      </div>

      <SessionFooter counts={state.counts} goldTotal={stats.data?.total ?? null} seed={state.seed} />
    </div>
  );
}
