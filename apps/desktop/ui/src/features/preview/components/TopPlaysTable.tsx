import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { TopPlayDto } from "@/ipc/bindings";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/shared/ui/table";
import { usePreviewFormat } from "../format";
import { Heading, type HeadingLevel } from "./Heading";

export function TopPlaysTable({ plays, level }: { plays: readonly TopPlayDto[]; level: HeadingLevel }) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  const headingId = useId();
  return (
    <section aria-labelledby={headingId} className="bg-card ring-border flex flex-col gap-3 rounded-xl p-5 shadow-lg shadow-black/20 ring-1">
      <Heading level={level} id={headingId}>
        {t("preview.topPlays.title")}
      </Heading>
      {plays.length === 0 ? (
        <p className="text-muted-foreground text-sm">{t("preview.topPlays.empty")}</p>
      ) : (
        <Table aria-labelledby={headingId} className="tabular-nums">
          <TableHeader>
            <TableRow>
              <TableHead scope="col">{t("preview.topPlays.chart")}</TableHead>
              <TableHead scope="col" className="text-right">
                {t("preview.topPlays.rate")}
              </TableHead>
              <TableHead scope="col" className="text-right">
                {t("preview.topPlays.goal")}
              </TableHead>
              <TableHead scope="col" className="text-right">
                {t("preview.topPlays.ssr")}
              </TableHead>
              <TableHead scope="col">{t("preview.topPlays.skillset")}</TableHead>
              <TableHead scope="col" className="text-right">
                {t("preview.topPlays.date")}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {plays.map((play) => (
              <TableRow key={play.playId}>
                <TableCell className="max-w-72">
                  <span className="block truncate font-medium" title={`${play.title} [${play.version}]`}>
                    {play.title}
                  </span>
                  <span className="text-muted-foreground block truncate text-xs">[{play.version}]</span>
                </TableCell>
                <TableCell className="text-right">{format.rate(play.rateMilli)}</TableCell>
                <TableCell className="text-right">{format.goal(play.goalPermyriad)}</TableCell>
                <TableCell className="text-osu-pink text-right font-semibold">{format.approx(play.overallCenti)}</TableCell>
                <TableCell>{format.skillset(play.dominantSkillset)}</TableCell>
                <TableCell className="text-muted-foreground text-right">
                  {play.playedAtMs === null ? t("preview.topPlays.noDate") : format.date(play.playedAtMs)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </section>
  );
}
