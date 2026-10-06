import { useTranslation } from "react-i18next";
import { Button } from "@/shared/ui/button";
import type { FlagToggle } from "../session";
import type { ThumbSide } from "../types";

interface FlagTogglesProps {
  flags: { mixed: boolean; unsure: boolean; thumbPref: ThumbSide | null };
  onToggle: (toggle: FlagToggle) => void;
}

const TOGGLES: readonly { toggle: FlagToggle; labelKey: string; shortcut: string }[] = [
  { toggle: "mixed", labelKey: "label.flags.mixed", shortcut: "m" },
  { toggle: "unsure", labelKey: "label.flags.unsure", shortcut: "?" },
  { toggle: "thumbLeft", labelKey: "label.flags.thumbLeft", shortcut: "tl" },
  { toggle: "thumbRight", labelKey: "label.flags.thumbRight", shortcut: "tr" },
];

function pressed(flags: FlagTogglesProps["flags"], toggle: FlagToggle): boolean {
  switch (toggle) {
    case "mixed":
      return flags.mixed;
    case "unsure":
      return flags.unsure;
    case "thumbLeft":
      return flags.thumbPref === "left";
    case "thumbRight":
      return flags.thumbPref === "right";
  }
}

export function FlagToggles({ flags, onToggle }: FlagTogglesProps) {
  const { t } = useTranslation();
  return (
    <div role="group" aria-label={t("label.flags.title")} className="grid grid-cols-2 gap-1.5">
      {TOGGLES.map(({ toggle, labelKey, shortcut }) => {
        const on = pressed(flags, toggle);
        return (
          <Button
            key={toggle}
            size="sm"
            variant={on ? "default" : "outline"}
            aria-pressed={on}
            onClick={() => {
              onToggle(toggle);
            }}
            className="justify-between"
          >
            {t(labelKey)}
            <kbd aria-hidden="true" className="font-mono text-[0.7rem] opacity-70">
              {shortcut}
            </kbd>
          </Button>
        );
      })}
    </div>
  );
}
