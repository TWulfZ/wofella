import { useTranslation } from "react-i18next";
import { Badge } from "@/shared/ui/badge";
import type { LabelWindow } from "../types";

export function ChartHeader({ window, round }: { window: LabelWindow; round: number }) {
  const { t } = useTranslation();
  return (
    <header className="flex flex-col gap-1.5">
      <p className="text-muted-foreground text-xs tracking-wide uppercase">{t("label.round", { round: round + 1 })}</p>
      <h2 className="text-lg leading-tight font-semibold break-words">{window.title}</h2>
      <p className="text-muted-foreground text-sm">{window.artist}</p>
      <div className="flex flex-wrap items-center gap-1.5">
        <Badge variant="secondary">{window.version}</Badge>
        {window.level !== null && <Badge variant="outline">{t("label.level", { level: window.level })}</Badge>}
        <Badge variant="outline">{t("label.stratum", { stratum: window.stratum })}</Badge>
        <Badge variant={window.played ? "default" : "ghost"}>
          {window.played ? t("label.played") : t("label.notPlayed")}
        </Badge>
      </div>
    </header>
  );
}
