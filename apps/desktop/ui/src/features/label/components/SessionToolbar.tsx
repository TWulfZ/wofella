import { ChevronLeft, ChevronRight, type LucideIcon, Radio, Shuffle, SkipForward } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/shared/ui/tooltip";

export type ToolbarAction = "previous" | "next" | "random" | "nowPlaying" | "skip";

/** Why an action is unavailable, as an i18n key; null when it can run. */
export type ToolbarState = Record<ToolbarAction, string | null>;

const ACTIONS: readonly { action: ToolbarAction; icon: LucideIcon; labelKey: string }[] = [
  { action: "previous", icon: ChevronLeft, labelKey: "label.toolbar.previous" },
  { action: "next", icon: ChevronRight, labelKey: "label.toolbar.next" },
  { action: "random", icon: Shuffle, labelKey: "label.toolbar.random" },
  { action: "nowPlaying", icon: Radio, labelKey: "label.toolbar.nowPlaying" },
  { action: "skip", icon: SkipForward, labelKey: "label.toolbar.skip" },
];

interface SessionToolbarProps {
  blocked: ToolbarState;
  onAction: (action: ToolbarAction) => void;
}

export function SessionToolbar({ blocked, onAction }: SessionToolbarProps) {
  const { t } = useTranslation();
  return (
    <TooltipProvider delayDuration={300}>
      <div
        role="toolbar"
        aria-label={t("label.toolbar.label")}
        className="bg-card/60 flex flex-wrap items-center gap-1.5 rounded-xl border p-1.5"
      >
        {ACTIONS.map(({ action, icon, labelKey }, i) => (
          <ToolbarButton
            key={action}
            icon={icon}
            label={t(labelKey)}
            reason={blocked[action] === null ? null : t(blocked[action])}
            // Skip stands apart: it is the only one that records an outcome.
            className={cn(action === "skip" && "ml-auto", i === 2 && "ml-2")}
            onClick={() => {
              onAction(action);
            }}
          />
        ))}
      </div>
    </TooltipProvider>
  );
}

interface ToolbarButtonProps {
  icon: LucideIcon;
  label: string;
  reason: string | null;
  className?: string;
  onClick: () => void;
}

/** aria-disabled rather than disabled, so the button stays focusable and hoverable and its reason can be read. */
function ToolbarButton({ icon: Icon, label, reason, className, onClick }: ToolbarButtonProps) {
  const reasonId = useId();
  const [hovered, setHovered] = useState(false);
  const blocked = reason !== null;
  const button = (
    <Button
      type="button"
      variant="ghost"
      size="lg"
      aria-label={label}
      aria-disabled={blocked}
      aria-describedby={blocked ? reasonId : undefined}
      onClick={() => {
        if (!blocked) {
          onClick();
        }
      }}
      className={cn("h-9 gap-2 px-3", blocked && "cursor-not-allowed opacity-45 hover:bg-transparent", className)}
    >
      <Icon aria-hidden className="size-4" />
      <span>{label}</span>
      {blocked && (
        <span id={reasonId} hidden>
          {reason}
        </span>
      )}
    </Button>
  );
  // Always wrapped, so a button that becomes blocked keeps its node and focus.
  return (
    <Tooltip open={blocked && hovered} onOpenChange={setHovered}>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      {blocked && <TooltipContent side="bottom">{reason}</TooltipContent>}
    </Tooltip>
  );
}
