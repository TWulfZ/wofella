import { useTranslation } from "react-i18next";
import { Badge } from "@/shared/ui/badge";
import type { WindowOrigin } from "../session";
import type { LabelWindow } from "../types";

function originKey(origin: WindowOrigin): string {
  switch (origin.kind) {
    case "plan":
      return "label.round";
    case "random":
      return "label.origin.random";
    case "nowPlaying":
      return origin.source === "osuWindow" ? "label.origin.osuWindow" : "label.origin.lastReplay";
  }
}

export function ChartHeader({ window, origin }: { window: LabelWindow; origin: WindowOrigin }) {
  const { t } = useTranslation();
  return (
    <header className="flex flex-col gap-1.5">
      <p className="text-muted-foreground text-xs tracking-wide uppercase">
        {t(originKey(origin), { round: origin.kind === "plan" ? origin.round + 1 : 0 })}
      </p>
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
