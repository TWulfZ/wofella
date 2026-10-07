import { ChevronLeft, ChevronRight, type LucideIcon, Radio, Shuffle } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/shared/ui/tooltip";

export type NavAction = "previous" | "next" | "random" | "nowPlaying";

/** Why an action is unavailable, as an i18n key; null when it can run. */
export type NavState = Record<NavAction, string | null>;

const ACTIONS: readonly { action: NavAction; icon: LucideIcon; labelKey: string }[] = [
  { action: "previous", icon: ChevronLeft, labelKey: "label.toolbar.previous" },
  { action: "next", icon: ChevronRight, labelKey: "label.toolbar.next" },
  { action: "random", icon: Shuffle, labelKey: "label.toolbar.random" },
  { action: "nowPlaying", icon: Radio, labelKey: "label.toolbar.nowPlaying" },
];

interface HeaderNavProps {
  blocked: NavState;
  onAction: (action: NavAction) => void;
}

export function HeaderNav({ blocked, onAction }: HeaderNavProps) {
  const { t } = useTranslation();
  return (
    <TooltipProvider delayDuration={300}>
      <div role="toolbar" aria-label={t("label.toolbar.label")} className="flex items-center gap-0.5">
        {ACTIONS.map(({ action, icon, labelKey }) => (
          <NavButton
            key={action}
            icon={icon}
            label={t(labelKey)}
            reason={blocked[action] === null ? null : t(blocked[action])}
            // Random and Now playing leave the plan; a gap sets them apart from walking it.
            className={cn(action === "random" && "ml-2")}
            onClick={() => {
              onAction(action);
            }}
          />
        ))}
      </div>
    </TooltipProvider>
  );
}

interface NavButtonProps {
  icon: LucideIcon;
  label: string;
  reason: string | null;
  className?: string | undefined;
  onClick: () => void;
}

/** aria-disabled rather than disabled, so the button stays focusable and hoverable and its reason can be read. */
function NavButton({ icon: Icon, label, reason, className, onClick }: NavButtonProps) {
  const reasonId = useId();
  const [open, setOpen] = useState(false);
  const blocked = reason !== null;
  // Always wrapped, so a button that becomes blocked keeps its node and focus.
  return (
    <Tooltip open={open} onOpenChange={setOpen}>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={label}
          aria-disabled={blocked}
          aria-describedby={blocked ? reasonId : undefined}
          onClick={() => {
            if (!blocked) {
              onClick();
            }
          }}
          className={cn("size-8", blocked && "cursor-not-allowed opacity-45 hover:bg-transparent", className)}
        >
          <Icon aria-hidden className="size-4" />
          {blocked && (
            <span id={reasonId} hidden>
              {reason}
            </span>
          )}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="flex-col items-start gap-0.5">
        <span className="font-semibold">{label}</span>
        {blocked && <span>{reason}</span>}
      </TooltipContent>
    </Tooltip>
  );
}
