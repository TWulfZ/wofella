import { useQuery } from "@tanstack/react-query";
import {
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent,
  type SyntheticEvent,
  useEffect,
  useEffectEvent,
  useReducer,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { useTranslation } from "react-i18next";
import { clampOsuSpeed, DEFAULT_STAGE_PARAMS, Playfield, useLoadedSkin } from "@/features/playfield";
import type { PatternDefDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { Button } from "@/shared/ui/button";
import { type AnswerError, parseAnswer } from "./answer";
import { ActionBar, type Action } from "./components/ActionBar";
import { ChartHeader } from "./components/ChartHeader";
import { FlagToggles } from "./components/FlagToggles";
import { PatternChips } from "./components/PatternChips";
import { SessionFooter } from "./components/SessionFooter";
import { SkinPicker } from "./components/SkinPicker";
import { Transport } from "./components/Transport";
import { isPatternActive, togglePattern, withoutThumb } from "./draft";
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
  labelStatsQuery,
  labelTaxonomyQuery,
  skinListQuery,
  useLabelMutations,
  useSkinFile,
} from "./queries";
import { type AudioInput, type ClosableAudioContext, type LoopSpliceParams, SectionPlayer } from "./sectionPlayer";
import { type FlagToggle, initialSession, sampleExclusion, sessionReducer, submitFlags, undoTarget } from "./session";
import { selectedSkinFolder, skinOptions } from "./skins";
import type { Anchor, LabelWindow, ThumbSide } from "./types";

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
const NO_INLINE = { toggleMixed: false, toggleUnsure: false, thumb: null } as const;

// Not a REPL word: the action bar handles it before the answer grammar sees it.
const CLEAR_COMMAND = "clear";

const ANSWER_ACTIONS: readonly Action[] = [
  { labelKey: "label.answer.noPattern", command: "x" },
  { labelKey: "label.answer.skip", command: "s" },
  { labelKey: "label.answer.undo", command: "u" },
  { labelKey: "label.answer.help", command: "h", variant: "ghost" },
  { labelKey: "label.answer.clear", command: CLEAR_COMMAND, hint: "Esc", variant: "ghost" },
];

const WINDOW_ACTIONS: readonly Action[] = [
  { labelKey: "label.reshape.widen", command: "w+" },
  { labelKey: "label.reshape.narrow", command: "w-" },
  { labelKey: "label.reshape.prev", command: "p" },
  { labelKey: "label.reshape.next", command: "n" },
];

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

interface Feedback {
  tone: "error" | "info";
  text: string;
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

const TEXT_INPUT_TYPES: ReadonlySet<string> = new Set(["text", "search", "number", "email", "url", "tel", "password"]);

function isTextField(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement) {
    return TEXT_INPUT_TYPES.has(target.type);
  }
  return target instanceof HTMLTextAreaElement || (target instanceof HTMLElement && target.isContentEditable);
}

function thumbSide(toggle: FlagToggle): ThumbSide | null {
  return toggle === "thumbLeft" ? "left" : toggle === "thumbRight" ? "right" : null;
}

function LabelSession({ keymode, seed, createAudioContext, params, defaultOsuSpeed, onRestart }: LabelSessionProps) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const [state, dispatch] = useReducer(sessionReducer, seed, initialSession);
  const [draft, setDraft] = useState("");
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [showHelp, setShowHelp] = useState(false);
  const [quit, setQuit] = useState(false);
  const [sampleAttempt, setSampleAttempt] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  // The window an action runs against, kept after the action moves past it: an Enter that lands before the next
  // render still holds that window in its closure, and isPending only flips on that render.
  const actedOn = useRef<LabelWindow | null>(null);

  const taxonomy = useQuery(labelTaxonomyQuery(keymode));
  const stats = useQuery(labelStatsQuery());
  const { sample, resolve, reshape, submit, undo } = useLabelMutations(keymode);
  const labelWindow = state.window;
  const anchor: Anchor | null = labelWindow?.anchor ?? null;
  const chart = useQuery(chartWindowQuery(anchor));
  const audio = useQuery(chartAudioQuery(anchor?.md5 ?? null));

  const sampledKey = useRef<string | null>(null);
  const sampleWindow = sample.mutate;
  useEffect(() => {
    if (state.done || state.window !== null) {
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
          dispatch(next === null ? { type: "sessionDone" } : { type: "windowLoaded", window: next });
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
    if (state.done) {
      player.release();
    } else if (md5 === null) {
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

  useEffect(() => {
    if (md5 !== null) {
      inputRef.current?.focus();
    }
  }, [md5, state.round]);

  const parsed = parseAnswer(draft);
  const inline = parsed.ok && parsed.answer.kind === "labels" ? parsed.answer : NO_INLINE;
  const effectiveFlags = submitFlags(state.flags, inline);
  const busy = resolve.isPending || submit.isPending || reshape.isPending || undo.isPending;

  const answerErrorText = (error: AnswerError): string =>
    error.code === "commandInLine"
      ? t("label.answerError.commandInLine", { word: error.word })
      : t(`label.answerError.${error.code}`);

  const focusAnswer = (): void => {
    inputRef.current?.focus();
  };

  const runLine = async (line: string): Promise<void> => {
    if (labelWindow === null || actedOn.current === labelWindow) {
      return;
    }
    const result = parseAnswer(line);
    if (!result.ok) {
      setFeedback({ tone: "error", text: answerErrorText(result.error) });
      return;
    }
    setFeedback(null);
    const answer = result.answer;
    // A command from a button must not wipe the chips picked so far.
    const fromDraft = line === draft;
    const consumeDraft = (): void => {
      if (fromDraft) {
        setDraft("");
      }
    };
    const toggleFlag = (toggle: FlagToggle): void => {
      dispatch({ type: "flagsToggled", toggle });
      consumeDraft();
    };
    actedOn.current = labelWindow;
    let movedOn = false;
    try {
      switch (answer.kind) {
        case "labels": {
          const patterns = answer.noPattern ? [] : await resolve.mutateAsync(answer.tokens);
          const { mixed, unsure, thumbPref } = submitFlags(state.flags, answer);
          const event = await submit.mutateAsync({
            anchor: labelWindow.anchor,
            patterns,
            noPattern: answer.noPattern,
            mixed,
            unsure,
            thumbPref,
          });
          dispatch({ type: "submitted", eventId: event.id });
          setDraft("");
          movedOn = true;
          break;
        }
        case "thumb":
          toggleFlag(answer.side === "left" ? "thumbLeft" : "thumbRight");
          break;
        case "mixed":
          toggleFlag("mixed");
          break;
        case "unsure":
          toggleFlag("unsure");
          break;
        case "skip":
          dispatch({ type: "skipped" });
          setDraft("");
          movedOn = true;
          break;
        case "undo": {
          const target = undoTarget(state);
          consumeDraft();
          if (target === null) {
            setFeedback({ tone: "info", text: t("label.nothingToUndo") });
            break;
          }
          await undo.mutateAsync(target);
          dispatch({ type: "undone", eventId: target });
          setFeedback({ tone: "info", text: t("label.undone") });
          break;
        }
        case "reshape": {
          const next = await reshape.mutateAsync({ anchor: labelWindow.anchor, op: answer.op });
          dispatch({ type: "windowReshaped", anchor: next });
          consumeDraft();
          movedOn = true;
          break;
        }
        case "help":
          setShowHelp((v) => !v);
          consumeDraft();
          break;
        case "leave":
          setQuit(true);
          dispatch({ type: "sessionDone" });
          movedOn = true;
          break;
      }
    } catch (e) {
      setFeedback({ tone: "error", text: errorText(e) });
    } finally {
      if (!movedOn) {
        actedOn.current = null;
      }
    }
  };

  // Outside a text field every printable key belongs to the answer line: single-key commands there would fire while
  // a pattern key such as `st` or `bu` is being typed.
  const onWindowKey = useEffectEvent((e: KeyboardEvent) => {
    if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey || e.isComposing) {
      return;
    }
    // Not printable, so they never reach the answer line and work wherever the focus is, as in game.
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
    const onButton = e.target instanceof Element && e.target.closest("button") !== null;
    if (onButton && (e.key === " " || e.key === "Enter")) {
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (!e.repeat) {
        void runLine(draft);
      }
      return;
    }
    if (e.key === " " && draft.trim() === "") {
      e.preventDefault();
      if (!e.repeat) {
        player.toggle();
      }
      return;
    }
    if (e.key.length === 1 && labelWindow !== null) {
      e.preventDefault();
      setDraft((d) => d + e.key);
      focusAnswer();
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

  const onAnswerKey = (e: ReactKeyboardEvent<HTMLInputElement>): void => {
    if (e.key === "Escape") {
      e.preventDefault();
      setDraft("");
      return;
    }
    // Implicit form submission fires on every auto-repeated Enter.
    if (e.key === "Enter" && e.repeat) {
      e.preventDefault();
      return;
    }
    if (e.key === " " && draft.trim() === "") {
      e.preventDefault();
      if (!e.repeat) {
        player.toggle();
      }
    }
  };

  const onSubmit = (e: SyntheticEvent): void => {
    e.preventDefault();
    void runLine(draft);
  };

  // Clicking a button, the playfield or bare space must not take focus from the answer line, or the next Enter would
  // press that button again. Form controls keep their focus so sliders still drag.
  const keepAnswerFocus = (e: MouseEvent): void => {
    if (e.target instanceof Element && e.target.closest("input, textarea, select") === null) {
      e.preventDefault();
    }
  };

  const onRun = (command: string): void => {
    if (command === CLEAR_COMMAND) {
      setDraft("");
      focusAnswer();
      return;
    }
    void runLine(command);
  };

  // Window flags, like the REPL's lone m/?/tl/tr: the draft keeps its own inline toggles.
  const onFlag = (toggle: FlagToggle): void => {
    const side = thumbSide(toggle);
    if (side === null || inline.thumb === null) {
      dispatch({ type: "flagsToggled", toggle });
      return;
    }
    // An inline tl/tr overrides the window's side on save, so the button only means something once it is gone.
    setDraft(withoutThumb(draft));
    dispatch({ type: "thumbSet", side: effectiveFlags.thumbPref === side ? null : side });
  };

  const onChip = (pattern: PatternDefDto): void => {
    setDraft((d) => togglePattern(d, pattern));
  };

  const footer = <SessionFooter counts={state.counts} goldTotal={stats.data?.total ?? null} seed={state.seed} />;

  if (state.done) {
    return (
      <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
        <h1 className="text-2xl font-semibold">{t("label.title")}</h1>
        <p className="text-lg">{quit ? t("label.done.quit") : t("label.done.exhausted")}</p>
        {footer}
        <Button className="self-start" onClick={onRestart}>
          {t("label.done.newSession")}
        </Button>
      </div>
    );
  }

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
      onMouseDown={keepAnswerFocus}
      className="grid h-[calc(100dvh-4rem)] min-h-[32rem] grid-cols-[minmax(0,1fr)_minmax(20rem,26rem)] grid-rows-[minmax(0,1fr)_auto] gap-x-6 gap-y-3 p-4"
    >
      <section aria-label={t("label.title")} className="flex min-h-0 flex-col gap-3">
        {labelWindow === null || chart.data === undefined ? (
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
              onSettle={focusAnswer}
            />
            <SkinPicker
              options={skinOptions(skinList.data, keymode)}
              folder={skinFolder}
              ready={skinList.data !== undefined}
              reloading={skinReloading}
              onChange={(folder) => {
                setSkinChoice({ folder });
                writeSkinChoice({ folder });
                focusAnswer();
              }}
              onReload={() => {
                void reloadSkin();
              }}
            />
            {skinNotices.map((notice) => (
              <p key={notice} role="status" className="text-muted-foreground bg-muted/60 self-start rounded-md px-2.5 py-1 text-xs">
                {notice}
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

      <aside className="flex min-h-0 flex-col gap-4 overflow-y-auto pr-1">
        {labelWindow !== null && <ChartHeader window={labelWindow} round={state.round} />}

        <form onSubmit={onSubmit} className="flex flex-col gap-1.5">
          <label htmlFor="label-answer" className="text-sm font-medium">
            {t("label.answer.label")}
          </label>
          <div className="flex gap-1.5">
            <input
              id="label-answer"
              ref={inputRef}
              value={draft}
              autoComplete="off"
              spellCheck={false}
              placeholder={t("label.answer.placeholder")}
              disabled={labelWindow === null}
              onChange={(e) => {
                setDraft(e.target.value);
              }}
              onKeyDown={onAnswerKey}
              className="border-input bg-background focus-visible:ring-ring/50 h-9 min-w-0 flex-1 rounded-md border px-2.5 font-mono text-sm outline-none focus-visible:ring-3"
            />
            <Button type="submit" size="lg" disabled={labelWindow === null || busy} aria-keyshortcuts="Enter">
              {t("label.answer.submit")}
            </Button>
          </div>
          {feedback !== null && (
            <p
              role={feedback.tone === "error" ? "alert" : "status"}
              className={feedback.tone === "error" ? "text-destructive text-sm" : "text-muted-foreground text-sm"}
            >
              {feedback.text}
            </p>
          )}
        </form>

        <section aria-label={t("label.patterns")} className="flex flex-col gap-2">
          <h3 className="text-sm font-medium">{t("label.patterns")}</h3>
          {taxonomy.isError ? (
            <p role="alert" className="text-destructive text-sm">
              {errorText(taxonomy.error)}
            </p>
          ) : taxonomy.data === undefined ? (
            <p className="text-muted-foreground text-sm">{t("common.loading")}</p>
          ) : (
            <PatternChips
              taxonomy={taxonomy.data}
              isActive={(pattern) => isPatternActive(draft, pattern)}
              onToggle={onChip}
            />
          )}
        </section>

        <section className="flex flex-col gap-2">
          <h3 className="text-sm font-medium">{t("label.flags.title")}</h3>
          <FlagToggles flags={effectiveFlags} onToggle={onFlag} />
        </section>

        <ActionBar
          label={t("label.answer.label")}
          actions={ANSWER_ACTIONS}
          disabled={labelWindow === null || busy}
          onRun={onRun}
        />

        <section className="flex flex-col gap-2">
          <h3 className="text-sm font-medium">{t("label.reshape.title")}</h3>
          <ActionBar
            label={t("label.reshape.title")}
            actions={WINDOW_ACTIONS}
            disabled={labelWindow === null || busy}
            onRun={onRun}
          />
        </section>

        {showHelp && (
          <ul className="text-muted-foreground flex list-disc flex-col gap-1 pl-4 text-xs">
            {(["labels", "noPattern", "flags", "commands", "hotkeys"] as const).map((key) => (
              <li key={key}>{t(`label.help.${key}`)}</li>
            ))}
          </ul>
        )}
      </aside>

      <div className="col-span-2">{footer}</div>
    </div>
  );
}
