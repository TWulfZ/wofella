import {
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
  type Ref,
  useEffect,
  useEffectEvent,
  useId,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";

export const HOLD_BUTTON_PARAMS = {
  /** Long enough that a stray click never confirms, short enough not to feel like a chore on every window. */
  defaultHoldMs: 1000,
  /** Ring redraw interval; the confirm itself runs on its own timer, so this only sets the ring's smoothness. */
  tickMs: 16,
} as const;

export type HoldButtonVariant = "primary" | "secondary" | "danger";

/** A hold started elsewhere (the screen's Enter shortcut) that still shows on this button's ring. */
export interface HoldButtonHandle {
  press(): void;
  release(): void;
}

export interface HoldButtonProps {
  label: string;
  icon: ReactNode;
  holdMs?: number | undefined;
  onConfirm: () => void;
  disabled?: boolean;
  variant?: HoldButtonVariant;
  className?: string;
  /** Ids of extra descriptions (why it is disabled), read after the hold instruction. */
  describedBy?: string | undefined;
  ref?: Ref<HoldButtonHandle> | undefined;
}

const BUTTON_VARIANT = { primary: "default", secondary: "secondary", danger: "destructive" } as const;
const RING_COLOUR: Record<HoldButtonVariant, string> = {
  primary: "text-primary",
  secondary: "text-foreground",
  danger: "text-destructive",
};

const RING_RADIUS = 21;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

type HoldSource = "pointer" | "key" | "remote";

interface ActiveHold {
  source: HoldSource;
  ticker: ReturnType<typeof setInterval>;
  timer: ReturnType<typeof setTimeout>;
  /** Set once confirmed; the release that follows only resets the ring. */
  done: boolean;
}

function isHoldKey(key: string): boolean {
  return key === "Enter" || key === " ";
}

/** Confirms only after a continuous hold, because the actions it guards (save, skip) cannot be taken back cheaply. */
export function HoldButton({
  label,
  icon,
  holdMs = HOLD_BUTTON_PARAMS.defaultHoldMs,
  onConfirm,
  disabled = false,
  variant = "primary",
  className,
  describedBy,
  ref,
}: HoldButtonProps) {
  const { t } = useTranslation();
  const descriptionId = useId();
  const [progress, setProgress] = useState(0);
  const hold = useRef<ActiveHold | null>(null);
  const confirm = useRef(onConfirm);
  useLayoutEffect(() => {
    confirm.current = onConfirm;
  });

  const stop = (): void => {
    const active = hold.current;
    if (active !== null) {
      clearInterval(active.ticker);
      clearTimeout(active.timer);
      hold.current = null;
    }
    setProgress(0);
  };

  // Becoming disabled (or unmounting) mid-hold must not let the pending timer confirm.
  useEffect(() => {
    if (disabled) {
      return;
    }
    return () => {
      const active = hold.current;
      if (active !== null) {
        clearInterval(active.ticker);
        clearTimeout(active.timer);
        hold.current = null;
      }
      setProgress(0);
    };
  }, [disabled]);

  // Holds driven from outside (the screen's Enter) or by a pointer on an unfocused button get no blur or release once
  // the app loses focus, so the timer would confirm on its own.
  const cancelAway = useEffectEvent(() => {
    if (hold.current !== null) {
      stop();
    }
  });
  useEffect(() => {
    const onBlur = (): void => {
      cancelAway();
    };
    const onVisibility = (): void => {
      if (document.visibilityState === "hidden") {
        cancelAway();
      }
    };
    window.addEventListener("blur", onBlur);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("blur", onBlur);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, []);

  const start = (source: HoldSource): void => {
    if (disabled) {
      return;
    }
    if (hold.current !== null) {
      // A confirmed hold whose release was lost must not swallow the next press.
      if (!hold.current.done) {
        return;
      }
      stop();
    }
    const startedAt = Date.now();
    const ticker = setInterval(() => {
      setProgress(Math.min(1, (Date.now() - startedAt) / holdMs));
    }, HOLD_BUTTON_PARAMS.tickMs);
    const timer = setTimeout(() => {
      clearInterval(ticker);
      const active = hold.current;
      if (active === null) {
        return;
      }
      active.done = true;
      setProgress(1);
      confirm.current();
    }, holdMs);
    hold.current = { source, ticker, timer, done: false };
  };

  const release = (source: HoldSource): void => {
    if (hold.current?.source === source) {
      stop();
    }
  };

  useImperativeHandle(ref, () => ({
    press: () => {
      start("remote");
    },
    release: () => {
      release("remote");
    },
  }));

  const onPointerDown = (e: PointerEvent<HTMLButtonElement>): void => {
    if (e.button === 0) {
      start("pointer");
    }
  };
  const onPointerEnd = (): void => {
    release("pointer");
  };
  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>): void => {
    if (!isHoldKey(e.key)) {
      return;
    }
    // The native activation would otherwise fire a click on Enter-down / Space-up.
    e.preventDefault();
    if (!e.repeat) {
      start("key");
    }
  };
  const onKeyUp = (e: KeyboardEvent<HTMLButtonElement>): void => {
    if (isHoldKey(e.key)) {
      e.preventDefault();
      release("key");
    }
  };

  const shown = disabled ? 0 : progress;

  return (
    <Button
      type="button"
      variant={BUTTON_VARIANT[variant]}
      size="icon-lg"
      aria-label={label}
      aria-describedby={describedBy === undefined ? descriptionId : `${descriptionId} ${describedBy}`}
      title={label}
      disabled={disabled}
      onPointerDown={onPointerDown}
      onPointerUp={onPointerEnd}
      onPointerLeave={onPointerEnd}
      onPointerCancel={onPointerEnd}
      onKeyDown={onKeyDown}
      onKeyUp={onKeyUp}
      onBlur={stop}
      onContextMenu={(e) => {
        e.preventDefault();
      }}
      className={cn("relative size-10 touch-none rounded-full active:not-aria-[haspopup]:translate-y-0 [&_svg:not([class*='size-'])]:size-5", className)}
    >
      {icon}
      {/* Linear and never motion-gated: the ring is the countdown itself, so reduced motion still needs it to advance. */}
      <svg
        aria-hidden
        data-testid="hold-ring"
        data-progress={shown}
        viewBox="0 0 48 48"
        className={cn(
          "pointer-events-none absolute -inset-1 size-12 -rotate-90",
          RING_COLOUR[variant],
          shown === 0 && "opacity-0",
        )}
      >
        <circle cx="24" cy="24" r={RING_RADIUS} fill="none" stroke="currentColor" strokeOpacity={0.25} strokeWidth="3" />
        <circle
          cx="24"
          cy="24"
          r={RING_RADIUS}
          fill="none"
          stroke="currentColor"
          strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray={RING_CIRCUMFERENCE}
          strokeDashoffset={RING_CIRCUMFERENCE * (1 - shown)}
        />
      </svg>
      <span id={descriptionId} className="sr-only">
        {t("label.hold.description", { count: holdMs / 1000, action: label.toLocaleLowerCase() })}
      </span>
    </Button>
  );
}
