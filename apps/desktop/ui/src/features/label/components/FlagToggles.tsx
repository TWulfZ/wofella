import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import type { FlagToggle } from "../session";
import type { ThumbSide } from "../types";

interface FlagTogglesProps {
  flags: { mixed: boolean; unsure: boolean; thumb: ThumbSide | null };
  disabled: boolean;
  /** Whether the active hand layout has a thumb column; a thumb side means nothing without one. */
  thumbs: boolean;
  onToggle: (toggle: FlagToggle) => void;
}

const TOGGLES: readonly { toggle: FlagToggle; labelKey: string; thumb: boolean }[] = [
  { toggle: "mixed", labelKey: "label.flags.mixed", thumb: false },
  { toggle: "unsure", labelKey: "label.flags.unsure", thumb: false },
  { toggle: "thumbLeft", labelKey: "label.flags.thumbLeft", thumb: true },
  { toggle: "thumbRight", labelKey: "label.flags.thumbRight", thumb: true },
];

function pressed(flags: FlagTogglesProps["flags"], toggle: FlagToggle): boolean {
  switch (toggle) {
    case "mixed":
      return flags.mixed;
    case "unsure":
      return flags.unsure;
    case "thumbLeft":
      return flags.thumb === "left";
    case "thumbRight":
      return flags.thumb === "right";
  }
}

export function FlagToggles({ flags, disabled, thumbs, onToggle }: FlagTogglesProps) {
  const { t } = useTranslation();
  return (
    <div role="group" aria-label={t("label.flags.title")} className="flex flex-wrap gap-1">
      {TOGGLES.filter(({ thumb }) => thumbs || !thumb).map(({ toggle, labelKey }) => {
        const on = pressed(flags, toggle);
        return (
          <Button
            key={toggle}
            type="button"
            size="sm"
            variant="outline"
            aria-pressed={on}
            disabled={disabled}
            onClick={() => {
              onToggle(toggle);
            }}
            className={cn(
              "rounded-full",
              on && "border-osu-blue/70 bg-osu-blue/15 text-osu-blue hover:bg-osu-blue/20 hover:text-osu-blue",
            )}
          >
            {t(labelKey)}
          </Button>
        );
      })}
    </div>
  );
}
