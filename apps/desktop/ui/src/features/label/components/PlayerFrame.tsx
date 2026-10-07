import { ChevronRight, X } from "lucide-react";
import {
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";

export const PLAYER_FRAME_PARAMS = {
  /** A video player's habit: long enough to reach a control, short enough not to sit over the notes. */
  idleMs: 2000,
  /** A hover peek shorter than this is a pointer passing by, not a viewer finding the settings. */
  settingsFoundDwellMs: 500,
  /** Only the pointer this near the bottom edge calls the controls: a share of the stage, floored for short stages. */
  controlsHoverZoneShare: 0.25,
  controlsHoverZoneMinPx: 144,
  /** One sway of the nudge's arrow, out or back: slow enough to read as a hint, not an alarm. */
  hintPeriodMs: 700,
} as const;

const FOCUSABLE = "button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex='-1'])";

export interface PlayerFrameProps {
  /** The stage; it takes the whole frame. */
  children: ReactNode;
  /** Laid over the stage's bottom edge, shown on pointer activity or keyboard focus. */
  controls: ReactNode;
  /** The side flyout's content. */
  settings: ReactNode;
  /** Laid over the stage's top edge, always shown. */
  notices?: ReactNode;
  /** Laid over the stage's middle; only its own content takes the pointer. */
  centre?: ReactNode;
  /** A first-run nudge on the settings tab; null or absent once the viewer has found the settings. */
  settingsHint?: string | null;
  /** Called when the viewer finds the settings: a press on the tab, a focused control, or a peek that lasts. */
  onSettingsOpen?: () => void;
  idleMs?: number;
  className?: string;
}

function useControlsVisibility(idleMs: number) {
  const [awake, setAwake] = useState(true);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [dragging, setDragging] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const arm = useCallback(() => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
    }
    timer.current = setTimeout(() => {
      timer.current = null;
      setAwake(false);
    }, idleMs);
  }, [idleMs]);

  const wake = useCallback(() => {
    setAwake(true);
    arm();
  }, [arm]);

  const sleep = useCallback(() => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    setAwake(false);
  }, []);

  // Shown on mount (the initial state), so the player can be found before it first fades.
  useEffect(() => {
    arm();
    return () => {
      if (timer.current !== null) {
        clearTimeout(timer.current);
      }
    };
  }, [arm]);

  useEffect(() => {
    if (!dragging) {
      return;
    }
    const end = (): void => {
      setDragging(false);
      wake();
    };
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    return () => {
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
    };
  }, [dragging, wake]);

  return {
    visible: awake || hovered || focused || dragging,
    wake,
    sleep,
    setHovered,
    setFocused,
    startDrag: () => {
      setDragging(true);
    },
  };
}

type FlyoutMode = "closed" | "peek" | "open";

/** Lazer's replay settings: an arrow tab on the stage's edge slides out a panel; hover peeks, a press opens it. */
interface SettingsFlyoutProps {
  children: ReactNode;
  hint: string | null;
  onOpen: (() => void) | undefined;
}

function SettingsFlyout({ children, hint, onOpen }: SettingsFlyoutProps) {
  const { t } = useTranslation();
  const panelId = useId();
  const hintId = useId();
  const [mode, setMode] = useState<FlyoutMode>("closed");
  const rootRef = useRef<HTMLDivElement>(null);
  const tabRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const label = t("label.player.settings");

  const foundTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cancelFound = (): void => {
    if (foundTimer.current !== null) {
      clearTimeout(foundTimer.current);
      foundTimer.current = null;
    }
  };
  useEffect(
    () => () => {
      if (foundTimer.current !== null) {
        clearTimeout(foundTimer.current);
      }
    },
    [],
  );
  const found = (): void => {
    cancelFound();
    onOpen?.();
  };

  const close = (returnFocus: boolean): void => {
    cancelFound();
    setMode("closed");
    if (returnFocus) {
      tabRef.current?.focus();
    }
  };

  // Only an open from the tab moves focus in; a peeked panel pinned by focusing one of its controls keeps that focus.
  const focusOnOpen = useRef(false);
  useEffect(() => {
    if (mode !== "open" || !focusOnOpen.current) {
      return;
    }
    focusOnOpen.current = false;
    panelRef.current?.querySelector<HTMLElement>(FOCUSABLE)?.focus();
  }, [mode]);

  useEffect(() => {
    if (mode === "closed") {
      return;
    }
    const onPointerDown = (e: PointerEvent): void => {
      if (e.target instanceof Node && rootRef.current?.contains(e.target) !== true) {
        cancelFound();
        setMode("closed");
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [mode]);

  const onPanelKeyDown = (e: KeyboardEvent<HTMLDivElement>): void => {
    if (e.key === "Escape") {
      // The screen clears the answer on Escape; this one only closes the panel.
      e.preventDefault();
      close(true);
      return;
    }
    if (e.key !== "Tab" || panelRef.current === null) {
      return;
    }
    const focusable = [...panelRef.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
    const first = focusable[0];
    const last = focusable.at(-1);
    if (first === undefined || last === undefined) {
      return;
    }
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  const open = mode !== "closed";
  const hinting = hint !== null && !open;
  return (
    <div
      ref={rootRef}
      data-testid="settings-flyout"
      onPointerLeave={() => {
        if (mode === "peek" && rootRef.current?.contains(document.activeElement) !== true) {
          cancelFound();
          setMode("closed");
        }
      }}
      // The strip spans the stage's height for the panel; only the tab and the panel take the pointer.
      className="pointer-events-none absolute inset-y-0 left-0 z-20 flex items-center"
    >
      {open && (
        <div
          ref={panelRef}
          id={panelId}
          role="dialog"
          aria-label={label}
          onKeyDown={onPanelKeyDown}
          onFocus={() => {
            if (mode === "peek") {
              found();
            }
            setMode("open");
          }}
          className={cn(
            // Translucent so the stage shows through while tuning it. The tint alone keeps foreground/80 text at 4.7:1
            // over a white stage (the test measures it); lighter tints fall under 4.5:1 over bright LN bodies.
            "bg-surface-raised/80 pointer-events-auto relative flex h-full w-72 max-w-[85%] flex-col gap-4 overflow-y-auto border-r p-4 shadow-xl backdrop-blur-sm",
            "motion-safe:animate-in motion-safe:slide-in-from-left-4 motion-safe:fade-in-0 motion-safe:duration-150",
          )}
        >
          <h3 className="font-display pr-8 text-xs font-bold tracking-widest uppercase">{label}</h3>
          {children}
          {/* Last in the DOM so Tab walks the settings first; drawn in the corner. */}
          <button
            type="button"
            aria-label={t("label.player.closeSettings")}
            title={t("label.player.closeSettings")}
            onClick={() => {
              close(true);
            }}
            className="text-foreground/80 hover:bg-muted hover:text-foreground focus-visible:ring-ring absolute top-3 right-3 grid size-7 place-items-center rounded-md outline-none focus-visible:ring-2"
          >
            <X aria-hidden className="size-4" />
          </button>
        </div>
      )}
      <button
        ref={tabRef}
        type="button"
        aria-label={label}
        title={label}
        aria-expanded={mode === "open"}
        aria-controls={open ? panelId : undefined}
        aria-describedby={hinting ? hintId : undefined}
        data-hint={hinting ? "true" : undefined}
        // Only the tab peeks: the flyout's root is a full-height strip that a pointer crosses on its way to the stage.
        onPointerEnter={() => {
          if (mode === "closed") {
            setMode("peek");
            cancelFound();
            foundTimer.current = setTimeout(found, PLAYER_FRAME_PARAMS.settingsFoundDwellMs);
          }
        }}
        onClick={() => {
          if (mode === "open") {
            close(false);
          } else {
            found();
            focusOnOpen.current = true;
            setMode("open");
          }
        }}
        className={cn(
          "bg-surface-raised/80 text-muted-foreground hover:text-foreground pointer-events-auto flex h-16 w-5 items-center justify-center rounded-r-md border border-l-0 outline-none backdrop-blur",
          "focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-1",
          // The static part of the nudge: what reduced-motion viewers get on its own.
          hinting && "text-primary border-primary/70 ring-primary/40 ring-2",
        )}
      >
        <ChevronRight
          aria-hidden
          data-testid={hinting ? "settings-hint-arrow" : undefined}
          style={hinting ? { animationDuration: `${String(PLAYER_FRAME_PARAMS.hintPeriodMs)}ms` } : undefined}
          className={cn(
            "size-4 motion-safe:transition-transform motion-safe:duration-150",
            open && "rotate-180",
            hinting &&
              "motion-safe:animate-out motion-safe:slide-out-to-right-1 motion-safe:repeat-infinite motion-safe:direction-alternate motion-safe:ease-in-out",
          )}
        />
      </button>
      {hinting && (
        <p
          id={hintId}
          className="bg-primary text-primary-foreground pointer-events-none absolute top-1/2 left-7 w-max max-w-56 -translate-y-1/2 rounded-md px-2.5 py-1.5 text-xs font-medium shadow-lg motion-safe:animate-in motion-safe:fade-in-0"
        >
          {hint}
        </p>
      )}
    </div>
  );
}

/** A video player's frame around the stage: controls over the bottom edge on activity, settings in a side flyout. */
export function PlayerFrame({
  children,
  controls,
  settings,
  notices,
  centre,
  settingsHint = null,
  onSettingsOpen,
  idleMs = PLAYER_FRAME_PARAMS.idleMs,
  className,
}: PlayerFrameProps) {
  const { t } = useTranslation();
  const visibility = useControlsVisibility(idleMs);
  const onControlsBlur = (e: FocusEvent<HTMLDivElement>): void => {
    if (!(e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget))) {
      visibility.setFocused(false);
      visibility.wake();
    }
  };
  return (
    <div
      data-testid="player-frame"
      onPointerLeave={visibility.sleep}
      className={cn("bg-background relative isolate min-h-0 flex-1 overflow-hidden rounded-xl border", className)}
    >
      {children}
      {notices !== undefined && (
        <div className="pointer-events-none absolute inset-x-0 top-0 z-10 flex flex-col items-center gap-1.5 p-2 *:pointer-events-auto">
          {notices}
        </div>
      )}
      {centre != null && (
        <div
          data-testid="player-centre"
          className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center *:pointer-events-auto"
        >
          {centre}
        </div>
      )}
      {/* A video player's habit: only the pointer near the bottom edge calls the controls, not every move over the stage. */}
      <div
        aria-hidden
        data-testid="controls-hover-zone"
        onPointerEnter={visibility.wake}
        onPointerMove={visibility.wake}
        style={{
          height: `${String(PLAYER_FRAME_PARAMS.controlsHoverZoneShare * 100)}%`,
          minHeight: PLAYER_FRAME_PARAMS.controlsHoverZoneMinPx,
        }}
        className="absolute inset-x-0 bottom-0 z-10"
      />
      <div
        role="group"
        aria-label={t("label.player.controls")}
        data-visible={visibility.visible}
        onPointerEnter={() => {
          visibility.setHovered(true);
        }}
        onPointerLeave={() => {
          visibility.setHovered(false);
          visibility.wake();
        }}
        onPointerDown={visibility.startDrag}
        onFocus={() => {
          visibility.setFocused(true);
        }}
        onBlur={onControlsBlur}
        className={cn(
          // pl-8 keeps the controls clear of the settings tab, which a short stage brings down into their row.
          "from-background/85 via-background/55 absolute inset-x-0 bottom-0 z-10 flex flex-col gap-2 bg-linear-to-t to-transparent pt-10 pr-4 pb-3 pl-8",
          "motion-safe:transition-opacity motion-safe:duration-200",
          // Faded, never removed: a keyboard user tabbing in must still land on the controls and bring them back.
          visibility.visible ? "opacity-100" : "pointer-events-none opacity-0",
        )}
      >
        {controls}
      </div>
      <SettingsFlyout hint={settingsHint} onOpen={onSettingsOpen}>
        {settings}
      </SettingsFlyout>
    </div>
  );
}
