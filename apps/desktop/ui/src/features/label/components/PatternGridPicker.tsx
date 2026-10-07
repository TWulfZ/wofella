import { useQuery } from "@tanstack/react-query";
import { LayoutGrid } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ChartWindow } from "@/features/playfield";
import { handLayoutQuery } from "@/features/preferences";
import type { PatternDefDto } from "@/ipc/bindings";
import { Button } from "@/shared/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/shared/ui/dialog";
import { labelPatternExamplesQuery } from "../queries";
import { PatternGrid, type PatternGridHandle } from "./PatternGrid";

export interface PatternGridPickerProps {
  keymode: number;
  taxonomy: readonly PatternDefDto[];
  /** The pattern id shown pressed, or null. */
  chosen: string | null;
  onChoose: (pattern: PatternDefDto) => void;
  disabled: boolean;
  /** The trigger's accessible name must contain its visible text (WCAG 2.5.3). */
  trigger: { label: string; text: ReactNode };
  title: string;
  description: string;
}

const NO_EXAMPLES: ReadonlyMap<string, ChartWindow> = new Map();

/** The Label screen's pattern grid as a one-pattern chooser: a card press chooses it and closes. */
export function PatternGridPicker({
  keymode,
  taxonomy,
  chosen,
  onChoose,
  disabled,
  trigger,
  title,
  description,
}: PatternGridPickerProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button type="button" variant="outline" size="sm" aria-label={trigger.label} disabled={disabled} className="max-w-56">
          <LayoutGrid aria-hidden />
          <span className="min-w-0 truncate">{trigger.text}</span>
        </Button>
      </DialogTrigger>
      <DialogContent
        closeLabel={t("common.close")}
        className="flex max-h-[88dvh] flex-col gap-4 sm:max-w-4xl"
        onEscapeKeyDown={(e) => {
          // Radix listens in the capture phase, before the grid's own Escape clears a typed search; that clear wins.
          const target = e.target;
          if (target instanceof HTMLInputElement && target.type === "search" && target.value !== "") {
            e.preventDefault();
          }
        }}
      >
        <DialogHeader className="pr-8">
          <DialogTitle className="font-display text-lg font-semibold">{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <PickerGrid
          keymode={keymode}
          taxonomy={taxonomy}
          chosen={chosen}
          onChoose={(pattern) => {
            onChoose(pattern);
            setOpen(false);
          }}
        />
      </DialogContent>
    </Dialog>
  );
}

interface PickerGridProps {
  keymode: number;
  taxonomy: readonly PatternDefDto[];
  chosen: string | null;
  onChoose: (pattern: PatternDefDto) => void;
}

function PickerGrid({ keymode, taxonomy, chosen, onChoose }: PickerGridProps) {
  // Same queries as the Label screen, so its cached examples are reused and the cards draw the viewer's layout.
  const handLayout = useQuery(handLayoutQuery(keymode));
  const examples = useQuery({
    ...labelPatternExamplesQuery(keymode, handLayout.data ?? null),
    enabled: !handLayout.isPending,
  });
  const gridRef = useRef<PatternGridHandle>(null);
  // Only the choice the dialog opened with: a new choice closes it, and the exit animation must not open another axis.
  const [openedWith] = useState(chosen);
  useEffect(() => {
    if (openedWith !== null) {
      // Axes start collapsed; focusPattern opens the chosen one for now only, leaving the saved layout and the search focus.
      gridRef.current?.focusPattern(openedWith, { moveFocus: false });
    }
  }, [openedWith]);
  return (
    <div className="-mx-4 min-h-0 flex-1 overflow-y-auto px-4 pb-1">
      <PatternGrid
        ref={gridRef}
        taxonomy={taxonomy}
        examples={examples.isError ? NO_EXAMPLES : examples.data}
        isActive={(pattern) => pattern.id === chosen}
        onToggle={onChoose}
      />
    </div>
  );
}
