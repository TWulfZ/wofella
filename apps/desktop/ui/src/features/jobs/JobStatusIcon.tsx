import { CircleCheck, CircleSlash, CircleX, LoaderCircle, type LucideIcon } from "lucide-react";
import { cn } from "cn";
import type { JobStatusDto } from "@/ipc/bindings";

const STATUS_ICON: Record<JobStatusDto, { icon: LucideIcon; className: string }> = {
  ok: { icon: CircleCheck, className: "text-success" },
  failed: { icon: CircleX, className: "text-destructive" },
  cancelled: { icon: CircleSlash, className: "text-muted-foreground" },
  queued: { icon: LoaderCircle, className: "text-osu-blue" },
  running: { icon: LoaderCircle, className: "text-osu-blue motion-safe:animate-spin" },
};

/** Decorative: callers always render the status text next to it, so state is never colour or shape alone. */
export function JobStatusIcon({ status, className }: { status: JobStatusDto; className?: string }) {
  const { icon: Icon, className: tone } = STATUS_ICON[status];
  return <Icon aria-hidden="true" className={cn(tone, className)} />;
}
