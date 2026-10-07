import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import type { FlagToggle } from "../session";
import type { ThumbSide } from "../types";

interface FlagTogglesProps {
  flags: { mixed: boolean; unsure: boolean; thumb: ThumbSide | null };
  disabled: boolean;
  onToggle: (toggle: FlagToggle) => void;
}

const TOGGLES: readonly { toggle: FlagToggle; labelKey: string }[] = [
  { toggle: "mixed", labelKey: "label.flags.mixed" },
  { toggle: "unsure", labelKey: "label.flags.unsure" },
  { toggle: "thumbLeft", labelKey: "label.flags.thumbLeft" },
  { toggle: "thumbRight", labelKey: "label.flags.thumbRight" },
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

export function FlagToggles({ flags, disabled, onToggle }: FlagTogglesProps) {
  const { t } = useTranslation();
  return (
    <div role="group" aria-label={t("label.flags.title")} className="flex flex-wrap gap-1">
      {TOGGLES.map(({ toggle, labelKey }) => {
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
