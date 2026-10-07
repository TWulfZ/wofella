import { Hourglass, Medal } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/shared/ui/badge";

/** No backend yet (ADR 0020 scope): the card only says what will come, never shows placeholder standings. */
export function RankingCard() {
  const { t } = useTranslation();
  const headingId = useId();
  return (
    <section
      aria-labelledby={headingId}
      className="bg-card ring-border osu-triangles relative flex items-start gap-4 rounded-xl p-5 shadow-lg shadow-black/20 ring-1"
    >
      <div className="bg-osu-purple/15 text-osu-purple grid size-11 shrink-0 place-items-center rounded-xl">
        <Medal aria-hidden="true" className="size-5" />
      </div>
      <div className="flex min-w-0 flex-col gap-1.5">
        <div className="flex flex-wrap items-center gap-2">
          <h2 id={headingId} className="font-display text-base font-semibold">
            {t("labelProgress.ranking.title")}
          </h2>
          <Badge variant="outline" className="border-osu-yellow/40 bg-osu-yellow/10 text-osu-yellow">
            <Hourglass aria-hidden="true" />
            {t("labelProgress.ranking.badge")}
          </Badge>
        </div>
        <p className="text-muted-foreground text-sm leading-relaxed">{t("labelProgress.ranking.body")}</p>
      </div>
    </section>
  );
}
