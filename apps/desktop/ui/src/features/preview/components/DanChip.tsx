import { Medal } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { DanEstimateDto } from "@/ipc/bindings";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";

export function DanChip({ dan }: { dan: DanEstimateDto }) {
  const { t } = useTranslation();
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="border-osu-blue/40 bg-osu-blue/10 text-osu-blue focus-visible:ring-ring inline-flex w-fit cursor-help items-center gap-1.5 rounded-full border px-3 py-1 text-sm font-semibold focus-visible:ring-2 focus-visible:outline-none"
        >
          <Medal aria-hidden="true" className="size-4" />
          <span className="sr-only">{t("preview.dan.label")}: </span>
          {t("preview.dan.value", { label: dan.label, third: t(`preview.dan.third.${dan.third}`) })}
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="max-w-64">
        {t("preview.dan.explain")}
      </TooltipContent>
    </Tooltip>
  );
}
