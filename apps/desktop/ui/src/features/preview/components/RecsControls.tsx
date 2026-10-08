import { useRef } from "react";
import { useTranslation } from "react-i18next";
import type { RecsModeDto } from "@/ipc/bindings";
import { cn } from "@/shared/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";
import { usePreviewFormat } from "../format";
import type { PickableSkillset } from "../model";

const MODES: readonly RecsModeDto[] = ["deficit", "push", "skillset"];

interface ModeTabsProps {
  mode: RecsModeDto;
  panelId: string;
  onChange: (mode: RecsModeDto) => void;
}

export function ModeTabs({ mode, panelId, onChange }: ModeTabsProps) {
  const { t } = useTranslation();
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const move = (from: number, step: number) => {
    const next = (from + step + MODES.length) % MODES.length;
    const target = MODES[next];
    if (target !== undefined) {
      onChange(target);
      tabs.current[next]?.focus();
    }
  };
  return (
    <div role="tablist" aria-label={t("preview.recs.modes.label")} className="bg-surface-raised grid grid-cols-3 gap-1 rounded-xl p-1">
      {MODES.map((m, index) => {
        const selected = m === mode;
        return (
          <button
            key={m}
            ref={(el) => {
              tabs.current[index] = el;
            }}
            type="button"
            role="tab"
            id={`${panelId}-${m}`}
            aria-selected={selected}
            aria-controls={panelId}
            tabIndex={selected ? 0 : -1}
            onClick={() => {
              onChange(m);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowRight") {
                e.preventDefault();
                move(index, 1);
              } else if (e.key === "ArrowLeft") {
                e.preventDefault();
                move(index, -1);
              }
            }}
            className={cn(
              "focus-visible:ring-ring focus-visible:ring-offset-surface-raised flex cursor-pointer flex-col items-start gap-0.5 rounded-lg px-3 py-2 text-left transition-colors focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none",
              selected ? "bg-primary text-primary-foreground shadow-[0_0_16px_-6px_var(--osu-pink)]" : "text-muted-foreground hover:bg-muted hover:text-foreground",
            )}
          >
            <span className="font-display text-sm font-bold">{t(`preview.recs.modes.${m}`)}</span>{" "}
            <span className={cn("text-xs", selected ? "text-primary-foreground/80" : "text-muted-foreground")}>
              {t(`preview.recs.modes.${m}Hint`)}
            </span>
          </button>
        );
      })}
    </div>
  );
}

interface SkillsetPickerProps {
  keymode: number;
  skillsets: readonly PickableSkillset[];
  value: string | null;
  onChange: (id: string) => void;
}

const OPTION =
  "has-[:focus-visible]:ring-ring has-[:focus-visible]:ring-offset-surface-raised rounded-md px-3 py-1.5 text-sm transition-colors has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-offset-2";

export function SkillsetPicker({ keymode, skillsets, value, onChange }: SkillsetPickerProps) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  return (
    <div role="radiogroup" aria-label={t("preview.recs.picker.label")} className="bg-surface-raised flex flex-wrap gap-1 rounded-lg p-1">
      {skillsets.map((s) => {
        const option = (
          <label
            key={s.id}
            className={cn(
              OPTION,
              s.enabled
                ? "bg-muted text-muted-foreground hover:text-foreground has-[:checked]:bg-osu-pink/15 has-[:checked]:text-osu-pink has-[:checked]:ring-osu-pink/50 cursor-pointer has-[:checked]:ring-1"
                : "text-muted-foreground/50 cursor-not-allowed line-through decoration-1",
            )}
          >
            <input
              type="radio"
              name="recs-skillset"
              value={s.id}
              className="sr-only"
              disabled={!s.enabled}
              checked={value === s.id}
              onChange={() => {
                onChange(s.id);
              }}
            />
            {format.skillset(s.id)}
          </label>
        );
        if (s.enabled) {
          return option;
        }
        return (
          <Tooltip key={s.id}>
            <TooltipTrigger asChild>{option}</TooltipTrigger>
            <TooltipContent side="bottom">{t("preview.recs.picker.notMeasured", { keymode })}</TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}
