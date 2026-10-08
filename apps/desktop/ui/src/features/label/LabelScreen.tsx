import { useQuery } from "@tanstack/react-query";
import {
  type CSSProperties,
  type MouseEvent,
  type ReactNode,
  useEffect,
  useEffectEvent,
  useReducer,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { useTranslation } from "react-i18next";
import {
  type ChartWindow,
  clampOsuSpeed,
  DEFAULT_STAGE_PARAMS,
  Playfield,
  type PlayfieldEffects,
  skinEffectSupport,
  useLoadedSkin,
} from "@/features/playfield";
import { handLayoutQuery, selectedSkinFolder, skinOptions } from "@/features/preferences";
import type { PatternDefDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { AnswerBar, type AnswerFeedback, type AnswerMode } from "./components/AnswerBar";
import { backgroundDataUrl, CHART_HEADER_PARAMS, ChartHeader } from "./components/ChartHeader";
import { ChartMsd } from "./components/ChartMsd";
import { ChartTimeline } from "./components/ChartTimeline";
import { ExportLabels } from "./components/ExportLabels";
import { HeaderNav, type NavAction, type NavState } from "./components/HeaderNav";
import { HOLD_BUTTON_PARAMS, type HoldButtonHandle } from "./components/HoldButton";
import { PanelResizer, usePanelWidth } from "./components/PanelResizer";
import { PatternGrid, type PatternGridHandle } from "./components/PatternGrid";
import { PlaybackSettings } from "./components/PlaybackSettings";
import { CentrePlayButton, PlayerControls } from "./components/PlayerControls";
import { PLAYER_FRAME_PARAMS, PlayerFrame } from "./components/PlayerFrame";
import { formatClock } from "./format";
import {
  clampPlaybackRate,
  hasStoredOsuSpeed,
  readOffsetMs,
  readPlaybackRate,
  readPlayfieldEffects,
  readScrollPrefs,
  readSettingsHintSeen,
  readSkinChoice,
  readZoom,
  scrollFromPrefs,
  type ScrollPrefs,
  writeOffsetMs,
  writeOsuSpeed,
  writePlaybackRate,
  writePlayfieldEffects,
  writePxPerMs,
  writeScrollKind,
  writeSettingsHintSeen,
  writeSkinChoice,
  writeZoom,
} from "./prefs";
import {
  chartAudioQuery,
  chartBackgroundQuery,
  chartDetailsQuery,
  chartTimelineQuery,
  chartWindowQuery,
  labelPatternExamplesQuery,
  labelStatsQuery,
  labelTaxonomyQuery,
  skinListQuery,
  useLabelExport,
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
import type { Anchor, Span } from "./types";

export interface LabelScreenParams {
  /** Audio heard before the window, so its first notes land in context. */
  prerollMs: number;
  postrollMs: number;
  /** px/ms mode speed until one is chosen. */
  defaultPxPerMs: number;
  /** osu! speed until one is chosen, when neither the caller nor the cfg (ManiaSpeed) gives one. */
  defaultOsuSpeed: number;
  loopSplice: LoopSpliceParams;
  /** Density buckets over the whole chart in the timeline. */
  timelineBuckets: number;
  /** How long Save and Skip (and the Enter shortcut) must be held. */
  holdMs: number;
  /** Pointer idle time over the preview before the playback controls fade. */
  controlsIdleMs: number;
  /** How long the controls show when a window's player appears. */
  controlsRevealMs: number;
  /** Back/forward and arrow-key step of the playhead, wrapping inside the section. */
  seekStepMs: number;
}

export const LABEL_SCREEN_PARAMS: LabelScreenParams = {
  prerollMs: 1000,
  postrollMs: 250,
  defaultPxPerMs: 1,
  defaultOsuSpeed: 20,
  loopSplice: { fadeMs: 30, gapMs: 150 },
  timelineBuckets: 240,
  holdMs: HOLD_BUTTON_PARAMS.defaultHoldMs,
  controlsIdleMs: PLAYER_FRAME_PARAMS.idleMs,
  controlsRevealMs: PLAYER_FRAME_PARAMS.revealMs,
  seekStepMs: 2000,
};

// osu!mania's in-game bindings: F3 slower, F4 faster.
const OSU_SPEED_KEYS: Readonly<Record<string, number>> = { F3: -1, F4: 1 };

const MS_PER_SECOND = 1000;

/** Room kept between a focused card and the bars pinned over the panel's edges (answer bar, compact title bar). */
const ANSWER_BAR_CLEARANCE_PX = 8;

export interface LabelScreenProps {
  keymode: number;
  /** Replays a session; by default each session draws one from the clock, as `wolluf label` does. */
  seed?: string;
  createAudioContext?: () => ClosableAudioContext;
  params?: LabelScreenParams;
  /** Used until a speed is chosen here, ahead of the cfg ManiaSpeed. */
  defaultOsuSpeed?: number;
  /** md5 of a chart to open first, ahead of the plan (the session list's "Inspect in Label screen"). */
  openChart?: string | null;
  /** Shown from the map card's counters. */
  countersDetails?: ReactNode;
  /** Drawn under the map card while the shown window's chart came from the session list (ADR 0020). */
  sessionMap?: ((map: SessionMapRef) => ReactNode) | undefined;
}

export interface SessionMapRef {
  md5: string;
  title: string;
}

function newSeed(): string {
  return String(Date.now());
}

export function LabelScreen(props: LabelScreenProps) {
  const { keymode, seed, createAudioContext, params = LABEL_SCREEN_PARAMS, countersDetails, sessionMap } = props;
  const defaultOsuSpeed = props.defaultOsuSpeed ?? null;
  // The handed-over chart opens with the first session only; New session starts from the plan.
  const [session, setSession] = useState(() => ({ id: 0, seed: seed ?? newSeed(), openChart: props.openChart ?? null }));
  return (
    <LabelSession
      key={session.id}
      keymode={keymode}
      seed={session.seed}
      openChart={session.openChart}
      countersDetails={countersDetails}
      sessionMap={sessionMap}
      createAudioContext={createAudioContext ?? (() => new AudioContext())}
      params={params}
      defaultOsuSpeed={defaultOsuSpeed}
      onRestart={() => {
        setSession((s) => ({ id: s.id + 1, seed: newSeed(), openChart: null }));
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
  openChart: string | null;
  countersDetails: ReactNode;
  sessionMap: ((map: SessionMapRef) => ReactNode) | undefined;
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

const NO_SPANS: readonly Span[] = [];

const TEXT_INPUT_TYPES: ReadonlySet<string> = new Set(["text", "search", "number", "email", "url", "tel", "password"]);

function isTextField(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement) {
    return TEXT_INPUT_TYPES.has(target.type);
  }
  return target instanceof HTMLTextAreaElement || (target instanceof HTMLElement && target.isContentEditable);
}

/** Keys these handle themselves (Enter presses a button, Space opens a select or ticks a box). */
const OWN_KEY_CONTROLS =
  "button, select, a[href], input[type='checkbox'], [role='separator'], [role='option'], [role='checkbox']";

/** Arrow keys these move themselves (sliders, range inputs, a select's options), on top of `OWN_KEY_CONTROLS`. */
const OWN_ARROW_CONTROLS = `${OWN_KEY_CONTROLS}, input, [role='slider']`;

// A video player's arrows: one seek step back or forward.
const SEEK_ARROWS: Readonly<Record<string, number>> = { ArrowLeft: -1, ArrowRight: 1 };

/** Hands focus back to the page, where Enter saves and Space plays. */
function releaseFocus(): void {
  const active = document.activeElement;
  if (active instanceof HTMLElement && active !== document.body) {
    active.blur();
  }
}

/**
 * The card's image under the panel's scrollbar gutter, which no child can paint into. `local` makes it scroll with the
 * content, and the size matches the card, so the strip continues the card's top edge; the scrim keeps the same fade.
 */
function gutterBackdrop(background: string | null, headerPx: number | null): CSSProperties {
  if (background === null || headerPx === null) {
    return {};
  }
  const size = `100% ${String(headerPx)}px`;
  return {
    backgroundImage: `linear-gradient(to bottom, transparent 15%, color-mix(in oklch, var(--background) 70%, transparent) 55%, var(--background)), url("${background}")`,
    backgroundSize: `${size}, ${size}`,
    backgroundRepeat: "no-repeat",
    backgroundAttachment: "local",
  };
}

function LabelSession(props: LabelSessionProps) {
  const { keymode, seed, createAudioContext, params, defaultOsuSpeed, openChart, countersDetails, sessionMap, onRestart } =
    props;
  const { t } = useTranslation();
  const errorText = useErrorText();
  const [state, dispatch] = useReducer(sessionReducer, seed, initialSession);
  const [feedback, setFeedback] = useState<AnswerFeedback | null>(null);
  const [notice, setNotice] = useState<AnswerFeedback | null>(null);
  const [sampleAttempt, setSampleAttempt] = useState(0);
  const panel = usePanelWidth();
  const [answerBarPx, setAnswerBarPx] = useState<number | null>(null);
  const [headerPx, setHeaderPx] = useState<number | null>(null);
  const patternScroll = useRef<HTMLDivElement>(null);
  // The entry a save runs against, kept after the save moves past it: an Enter that lands before the next render
  // still holds that entry in its closure, and isPending only flips on that render.
  const actedOn = useRef<HistoryEntry | null>(null);
  // Only the newest move's or resize's reply is applied, so keys pressed faster than the IPC round trip end where the
  // last one aimed.
  const reshapeSeq = useRef(0);
  const [pendingSpan, setPendingSpan] = useState<{ cursor: number; t0Ms: number; t1Ms: number } | null>(null);
  const saveHold = useRef<HoldButtonHandle>(null);
  const patternGrid = useRef<PatternGridHandle>(null);

  const taxonomy = useQuery(labelTaxonomyQuery(keymode));
  const handLayout = useQuery(handLayoutQuery(keymode));
  // Waits for the preference so the window is not fetched twice; an unreadable one draws the profile default.
  const layoutSettled = !handLayout.isPending;
  const layoutId = handLayout.data ?? null;
  const examples = useQuery({ ...labelPatternExamplesQuery(keymode, layoutId), enabled: layoutSettled });
  const stats = useQuery(labelStatsQuery());
  const { sample, random, nowPlaying, windowAt, move, resize, submit, undo } = useLabelMutations();
  const exportLabels = useLabelExport(keymode);
  const entry = currentEntry(state);
  const labelWindow = entry?.window ?? null;
  const anchor: Anchor | null = labelWindow?.anchor ?? null;
  const chart = useQuery(chartWindowQuery(layoutSettled ? anchor : null, layoutId));
  const audio = useQuery(chartAudioQuery(anchor?.md5 ?? null));
  const background = useQuery(chartBackgroundQuery(anchor?.md5 ?? null));
  const details = useQuery(chartDetailsQuery(anchor?.md5 ?? null));
  const timeline = useQuery(chartTimelineQuery(keymode, anchor?.md5 ?? null, params.timelineBuckets));
  const layoutThumbs = chart.data?.layout.columns.some((column) => column.finger === "thumb");
  // A new chart's window is undefined while it loads; holding the last layout's answer keeps the flag row from
  // jumping on every window. Before the first window it shows them, as it did before layouts were consulted.
  const [lastThumbs, setLastThumbs] = useState(true);
  if (layoutThumbs !== undefined && layoutThumbs !== lastThumbs) {
    setLastThumbs(layoutThumbs);
  }
  const thumbs = layoutThumbs ?? lastThumbs;
  const sampling = isSampling(state);

  // The plan waits for the handed-over chart, so it is the session's first window either way.
  const [opening, setOpening] = useState(openChart !== null);
  const openRequested = useRef(false);
  const openWindowAt = windowAt.mutate;
  const openFailed = useEffectEvent((e: unknown) => {
    setNotice({ tone: "error", text: errorText(e) });
  });
  useEffect(() => {
    if (openChart === null || openRequested.current) {
      return;
    }
    openRequested.current = true;
    openWindowAt(
      { keymode, md5: openChart, seed, windowMs: null, exclude: [] },
      {
        onSuccess: (next) => {
          dispatch({ type: "windowLoaded", window: next, origin: { kind: "session" } });
        },
        onError: openFailed,
        onSettled: () => {
          setOpening(false);
        },
      },
    );
  }, [openChart, keymode, seed, openWindowAt]);

  const sampledKey = useRef<string | null>(null);
  const sampleWindow = sample.mutate;
  useEffect(() => {
    if (opening || !isSampling(state)) {
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
  }, [state, keymode, sampleAttempt, sampleWindow, opening]);

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

  // The first-run nudge on the settings tab ends once the viewer finds the flyout or changes a setting another way.
  const [settingsHintSeen, setSettingsHintSeen] = useState(readSettingsHintSeen);
  const settingsFound = (): void => {
    if (!settingsHintSeen) {
      setSettingsHintSeen(true);
      writeSettingsHintSeen();
    }
  };

  const [offsetMs, setOffsetMs] = useState(readOffsetMs);
  useEffect(() => {
    player.setOffsetMs(offsetMs);
  }, [player, offsetMs]);
  const [rate, setRate] = useState(readPlaybackRate);
  useEffect(() => {
    player.setRate(rate);
  }, [player, rate]);
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
  const [effects, setEffects] = useState(readPlayfieldEffects);
  const updateEffects = (patch: Partial<PlayfieldEffects>): void => {
    const next = { ...effects, ...patch };
    setEffects(next);
    writePlayfieldEffects(next);
  };
  const updateScroll = (patch: Partial<ScrollPrefs>): void => {
    settingsFound();
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
  };

  const reshaping = move.isPending || resize.isPending;
  const busy = submit.isPending || reshaping || undo.isPending || random.isPending || nowPlaying.isPending;

  const save = async (): Promise<void> => {
    const payload = submitPayload(state);
    // During a move or resize the anchor is about to change; saving now would store the window being left.
    if (entry === null || payload === null || actedOn.current === entry || reshaping) {
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

  const runReshape = async (span: Span, request: (anchor: Anchor) => Promise<Anchor>): Promise<void> => {
    if (labelWindow === null) {
      return;
    }
    const { cursor } = state;
    reshapeSeq.current += 1;
    const seq = reshapeSeq.current;
    setPendingSpan({ cursor, ...span });
    setFeedback(null);
    try {
      const next = await request(labelWindow.anchor);
      if (seq === reshapeSeq.current) {
        dispatch({ type: "windowMoved", cursor, anchor: next });
      }
    } catch (e) {
      if (seq === reshapeSeq.current) {
        setFeedback({ tone: "error", text: errorText(e) });
      }
    } finally {
      if (seq === reshapeSeq.current) {
        setPendingSpan(null);
      }
    }
  };

  const runMove = (t0Ms: number): void => {
    if (anchor === null) {
      return;
    }
    void runReshape({ t0Ms, t1Ms: t0Ms + anchor.t1Ms - anchor.t0Ms }, (from) => move.mutateAsync({ anchor: from, t0Ms }));
  };

  const runResize = (span: Span): void => {
    void runReshape(span, (from) => resize.mutateAsync({ anchor: from, ...span }));
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

  const onNav = (action: NavAction): void => {
    setNotice(null);
    setFeedback(null);
    switch (action) {
      case "previous":
        dispatch({ type: "moved", to: "previous" });
        break;
      case "next":
        dispatch({ type: "moved", to: "next" });
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
        updateScroll({ osuSpeed });
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
    const seekDirection = SEEK_ARROWS[e.key];
    if (seekDirection !== undefined) {
      if (!(e.target instanceof Element && e.target.closest(OWN_ARROW_CONTROLS) !== null)) {
        e.preventDefault();
        // Not repeat-gated: holding the arrow scrubs, as in a video player.
        player.skip(seekDirection * params.seekStepMs);
      }
      return;
    }
    if (e.target instanceof Element && e.target.closest(OWN_KEY_CONTROLS) !== null) {
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      // Runs Save's own hold, so a tap never saves and the ring shows the countdown.
      if (!e.repeat) {
        saveHold.current?.press();
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
    // Wherever the key is released, a hold the shortcut started ends with it.
    const release = (e: KeyboardEvent): void => {
      if (e.key === "Enter") {
        saveHold.current?.release();
      }
    };
    window.addEventListener("keydown", listener);
    window.addEventListener("keyup", release);
    return () => {
      window.removeEventListener("keydown", listener);
      window.removeEventListener("keyup", release);
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
  const blocked: NavState = {
    previous: waiting ?? (state.cursor === 0 ? "label.toolbar.firstWindow" : null),
    next: waiting ?? planFinished,
    random: waiting,
    nowPlaying: waiting,
  };
  const canSkip = waiting === null && entry !== null && entry.status.kind !== "saved";
  const answerMode: AnswerMode =
    entry === null
      ? { kind: "none" }
      : entry.status.kind === "saved"
        ? { kind: "saved", canUndo: canUndo(state) }
        : { kind: "edit", skipped: entry.status.kind === "skipped", canSave: canSave(state) };
  const shownAnswer = entry?.status.kind === "saved" ? entry.status.answer : state.answer;

  const timelineWindow: Span | null =
    anchor === null
      ? null
      : pendingSpan?.cursor === state.cursor
        ? { t0Ms: pendingSpan.t0Ms, t1Ms: pendingSpan.t1Ms }
        : { t0Ms: anchor.t0Ms, t1Ms: anchor.t1Ms };
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
  // Until the chosen skin arrives the support describes the procedural stage, whose gaps are not the skin's.
  const skinLoading = skinFolder !== null && skin === null && !skinFile.isError;
  const skinProps = skin === null ? {} : { skin, hitPosition: skin.hitPosition, columnWidths: skin.columnWidth };
  const audioNotice = audio.isError
    ? errorText(audio.error)
    : playback.notice === "decodeFailed"
      ? t("label.audio.decodeFailed")
      : null;

  const nav = <HeaderNav blocked={blocked} onAction={onNav} />;
  const notices = [
    ...skinNotices,
    ...(audioNotice === null ? [] : [audioNotice]),
  ];

  return (
    <div
      onMouseDown={keepFocus}
      className={cn(
        // The right gutter is the job tray's edge tab's, which would otherwise sit over the panel's scrollbar.
        "flex h-[calc(100dvh-4rem)] min-h-[32rem] gap-2 py-4 pr-[calc(var(--job-tray-tab-w)+0.25rem)] pl-4",
        panel.dragging && "cursor-col-resize select-none",
      )}
    >
      <section aria-label={t("label.title")} className="flex min-w-0 flex-1 flex-col">
        {state.done && entry === null ? (
          <div role="status" className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
            <p className="text-lg">{t("label.done.exhausted")}</p>
            <p className="text-muted-foreground max-w-sm text-sm">{t("label.done.hint")}</p>
            <Button variant="outline" onClick={onRestart}>
              {t("label.done.newSession")}
            </Button>
            <ExportLabels exportLabels={exportLabels} />
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
          <PlayerFrame
            idleMs={params.controlsIdleMs}
            revealMs={params.controlsRevealMs}
            onStageClick={() => {
              player.toggle();
            }}
            settingsHint={settingsHintSeen ? null : t("label.player.hint")}
            onSettingsOpen={settingsFound}
            centre={
              playback.playing ? undefined : (
                <CentrePlayButton
                  onPlay={() => {
                    player.toggle();
                  }}
                />
              )
            }
            notices={
              notices.length === 0
                ? undefined
                : notices.map((text) => (
                    <p
                      key={text}
                      role="status"
                      className="bg-surface-raised/90 text-muted-foreground max-w-[90%] rounded-md border px-2.5 py-1 text-xs backdrop-blur"
                    >
                      {text}
                    </p>
                  ))
            }
            controls={
              <PlayerControls
                playing={playback.playing}
                loading={playback.loading}
                onToggle={() => {
                  player.toggle();
                }}
                clock={playback.clock}
                windowStartMs={anchor?.t0Ms ?? 0}
                range={rangeText}
                duration={durationText}
                rate={rate}
                onSkip={(deltaMs) => {
                  player.skip(deltaMs);
                }}
                skipMs={params.seekStepMs}
                timeline={
                  timelineWindow !== null && (
                    <ChartTimeline
                      span={
                        timeline.data ?? {
                          firstMs: chart.data.chartSpan.firstMs,
                          // The chart window's span ends on the last row; a window may reach one past it.
                          endMs: chart.data.chartSpan.endMs + 1,
                        }
                      }
                      window={timelineWindow}
                      density={timeline.data?.density ?? null}
                      labelled={timeline.data?.labelled ?? NO_SPANS}
                      locked={entry?.status.kind === "saved"}
                      onMove={runMove}
                      onResize={runResize}
                      playhead={{
                        positionMs: player.positionMs,
                        loop: { startMs: loopStart, endMs: loopEnd },
                        stepMs: params.seekStepMs,
                        onSeek: (chartMs) => {
                          player.seek(chartMs);
                        },
                      }}
                    />
                  )
                }
              />
            }
            settings={
              <>
                <PlaybackSettings
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
                  rate={rate}
                  onRate={(value) => {
                    const next = clampPlaybackRate(value);
                    setRate(next);
                    writePlaybackRate(next);
                  }}
                  effects={effects}
                  onEffects={updateEffects}
                  effectSupport={skinEffectSupport(skin)}
                  skinLoading={skinLoading}
                  skin={{
                    options: skinOptions(skinList.data, keymode),
                    folder: skinFolder,
                    ready: skinList.data !== undefined,
                    reloading: skinReloading,
                    onChange: (folder) => {
                      setSkinChoice({ folder });
                      writeSkinChoice({ folder });
                    },
                    onReload: () => {
                      void reloadSkin();
                    },
                  }}
                  seed={state.seed}
                />
                <ExportLabels exportLabels={exportLabels} />
              </>
            }
          >
            <div data-testid="playfield" className="flex size-full justify-center">
              <Playfield
                window={chart.data}
                clock={playback.clock}
                position={player.pausedAtMs}
                scroll={scrollFromPrefs(effectiveScroll)}
                zoom={zoom}
                effects={effects}
                {...skinProps}
                className="h-full w-full"
              />
            </div>
          </PlayerFrame>
        )}
      </section>

      <PanelResizer panel={panel} />

      <aside data-testid="pattern-panel" style={{ width: panel.width }} className="flex min-h-0 shrink-0 flex-col">
        {entry === null && (
          // Between windows (sampling, plan finished) the navigation still has to be reachable.
          <div className="bg-card flex w-full shrink-0 items-center rounded-t-xl border-b px-2 py-1.5">{nav}</div>
        )}
        <div
          ref={patternScroll}
          data-testid="pattern-scroll"
          style={{
            scrollPaddingTop: CHART_HEADER_PARAMS.compactBarPx + ANSWER_BAR_CLEARANCE_PX,
            scrollPaddingBottom: answerBarPx === null ? undefined : answerBarPx + ANSWER_BAR_CLEARANCE_PX,
            ...gutterBackdrop(entry === null ? null : backgroundDataUrl(background.data), headerPx),
          }}
          className="flex min-h-0 flex-1 flex-col overflow-y-auto rounded-t-xl [scrollbar-gutter:stable]"
        >
          {entry !== null && (
            <ChartHeader
              window={entry.window}
              origin={entry.origin}
              background={backgroundDataUrl(background.data)}
              details={details.data}
              difficulty={<ChartMsd md5={entry.window.anchor.md5} />}
              nav={nav}
              counters={{
                labelled: state.counts.labelled,
                skipped: state.counts.skipped,
                undone: state.counts.undone,
                gold: stats.data?.total ?? 0,
              }}
              scrollRoot={patternScroll}
              onBlockSize={setHeaderPx}
              countersDetails={countersDetails}
            />
          )}
          <div className="flex flex-1 flex-col gap-4 pt-3 pr-2 pl-1">
            {entry?.origin.kind === "session" &&
              sessionMap?.({ md5: entry.window.anchor.md5, title: entry.window.title })}
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
                  ref={patternGrid}
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
              thumbs={thumbs}
              onSave={() => {
                void save();
              }}
              onClear={clearAnswer}
              onUndo={() => {
                void runUndo();
              }}
              onSkip={() => {
                setNotice(null);
                setFeedback(null);
                dispatch({ type: "skipped" });
              }}
              canSkip={canSkip}
              onChipFocus={(id, how) => {
                patternGrid.current?.focusPattern(id, how);
              }}
              holdMs={params.holdMs}
              saveRef={saveHold}
              onBlockSize={setAnswerBarPx}
            />
          </div>
        </div>
      </aside>
    </div>
  );
}
