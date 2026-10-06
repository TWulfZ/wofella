import * as React from "react"
import { cn } from "cn"
import { Progress as ProgressPrimitive } from "radix-ui"

function Progress({
  className,
  value,
  ...props
}: React.ComponentProps<typeof ProgressPrimitive.Root>) {
  const indeterminate = value === null || value === undefined
  return (
    <ProgressPrimitive.Root
      data-slot="progress"
      className={cn(
        "relative flex h-1 w-full items-center overflow-x-hidden rounded-full bg-muted",
        className
      )}
      // Forwarded so Radix sets aria-valuenow; without it every bar is announced as indeterminate.
      value={value}
      {...props}
    >
      <ProgressPrimitive.Indicator
        data-slot="progress-indicator"
        className={cn(
          "h-full bg-linear-to-r from-osu-pink to-osu-purple",
          // Without a total, a partial segment reads as "working" without claiming any amount done.
          indeterminate
            ? "w-1/3 opacity-60 motion-safe:animate-progress-slide"
            : "w-full origin-left transition-transform duration-300"
        )}
        // scaleX, not translateX: the visible bar then always shows the whole pink-to-purple gradient.
        style={indeterminate ? undefined : { transform: `scaleX(${value / 100})` }}
      />
    </ProgressPrimitive.Root>
  )
}

export { Progress }
