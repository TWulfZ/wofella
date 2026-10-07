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
function SettingsFlyout({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const panelId = useId();
  const [mode, setMode] = useState<FlyoutMode>("closed");
  const rootRef = useRef<HTMLDivElement>(null);
  const tabRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const label = t("label.player.settings");

  const close = (returnFocus: boolean): void => {
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
  return (
    <div
      ref={rootRef}
      data-testid="settings-flyout"
      onPointerEnter={() => {
        if (mode === "closed") {
          setMode("peek");
        }
      }}
      onPointerLeave={() => {
        if (mode === "peek" && rootRef.current?.contains(document.activeElement) !== true) {
          setMode("closed");
        }
      }}
      className="absolute inset-y-0 left-0 z-20 flex items-center"
    >
      {open && (
        <div
          ref={panelRef}
          id={panelId}
          role="dialog"
          aria-label={label}
          onKeyDown={onPanelKeyDown}
          onFocus={() => {
            setMode("open");
          }}
          className={cn(
            "bg-surface-raised/95 relative flex h-full w-72 max-w-[85%] flex-col gap-4 overflow-y-auto border-r p-4 shadow-xl backdrop-blur",
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
            className="text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:ring-ring absolute top-3 right-3 grid size-7 place-items-center rounded-md outline-none focus-visible:ring-2"
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
        onClick={() => {
          if (mode === "open") {
            close(false);
          } else {
            focusOnOpen.current = true;
            setMode("open");
          }
        }}
        className={cn(
          "bg-surface-raised/80 text-muted-foreground hover:text-foreground flex h-16 w-5 items-center justify-center rounded-r-md border border-l-0 outline-none backdrop-blur",
          "focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-1",
        )}
      >
        <ChevronRight
          aria-hidden
          className={cn("size-4 motion-safe:transition-transform motion-safe:duration-150", open && "rotate-180")}
        />
      </button>
    </div>
  );
}

/** A video player's frame around the stage: controls over the bottom edge on activity, settings in a side flyout. */
export function PlayerFrame({
  children,
  controls,
  settings,
  notices,
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
      onPointerMove={visibility.wake}
      onPointerLeave={visibility.sleep}
      className={cn("bg-background relative isolate min-h-0 flex-1 overflow-hidden rounded-xl border", className)}
    >
      {children}
      {notices !== undefined && (
        <div className="pointer-events-none absolute inset-x-0 top-0 z-10 flex flex-col items-center gap-1.5 p-2 *:pointer-events-auto">
          {notices}
        </div>
      )}
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
          "from-background/85 via-background/55 absolute inset-x-0 bottom-0 z-10 flex flex-col gap-2 bg-linear-to-t to-transparent px-4 pt-10 pb-3",
          "motion-safe:transition-opacity motion-safe:duration-200",
          // Faded, never removed: a keyboard user tabbing in must still land on the controls and bring them back.
          visibility.visible ? "opacity-100" : "pointer-events-none opacity-0",
        )}
      >
        {controls}
      </div>
      <SettingsFlyout>{settings}</SettingsFlyout>
    </div>
  );
}
