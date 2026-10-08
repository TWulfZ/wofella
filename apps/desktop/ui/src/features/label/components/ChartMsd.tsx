import { useQuery } from "@tanstack/react-query";
import { ChevronDown } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ChartMsdDto, MsdStatusDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { cn } from "@/shared/lib/utils";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/shared/ui/table";
import { chartMsdQuery } from "../queries";

// ChartMsdDto::skillsets puts Overall first.
const OVERALL = 0;
const NOMINAL_RATE_MILLI = 1000;

const STATUS_TONE: Record<MsdStatusDto, string> = {
  rated: "bg-success/15 text-success",
  ln_heavy: "bg-osu-yellow/15 text-osu-yellow",
  calc_rejected: "bg-destructive/15 text-destructive",
  pending: "bg-muted text-muted-foreground",
};

function useMsdFormat() {
  const { t, i18n } = useTranslation();
  const msd = new Intl.NumberFormat(i18n.language, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  const rate = new Intl.NumberFormat(i18n.language, { minimumFractionDigits: 1, maximumFractionDigits: 3 });
  const percent = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 });
  return {
    // "≈" because these are MinaCalc's numbers on an osu! chart, not calibrated to this player (ADR 0024).
    msd: (centi: number) => t("label.msd.approx", { value: msd.format(centi / 100) }),
    rate: (rateMilli: number) => t("label.msd.rateValue", { rate: rate.format(rateMilli / 1000) }),
    percent: (permille: number) => percent.format(permille / 10),
  };
}

function skillsetName(id: string, t: (key: string) => string, exists: (key: string) => boolean): string {
  const key = `label.msd.skillset.${id}`;
  return exists(key) ? t(key) : id;
}

function StatusNote({ dto }: { dto: ChartMsdDto }) {
  const { t } = useTranslation();
  const format = useMsdFormat();
  switch (dto.status) {
    case "rated":
      return null;
    case "ln_heavy":
      return (
        <p className="text-muted-foreground text-sm">
          {t("label.msd.note.ln_heavy", { percent: format.percent(dto.holdSharePermille) })}
        </p>
      );
    case "calc_rejected":
    case "pending":
      return <p className="text-muted-foreground text-sm">{t(`label.msd.note.${dto.status}`)}</p>;
  }
}

function RateTable({ dto }: { dto: ChartMsdDto }) {
  const { t, i18n } = useTranslation();
  const format = useMsdFormat();
  return (
    <Table aria-label={t("label.msd.table")} className="tabular-nums">
      <TableHeader>
        <TableRow>
          <TableHead scope="col">{t("label.msd.rate")}</TableHead>
          {dto.skillsets.map((id) => (
            <TableHead key={id} scope="col" className="text-right">
              {skillsetName(id, t, (key) => i18n.exists(key))}
            </TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody>
        {dto.rates.map((rate) => (
          <TableRow key={rate.rateMilli} className={cn(rate.rateMilli === NOMINAL_RATE_MILLI && "bg-muted/40")}>
            <TableHead scope="row" className="font-medium">
              {format.rate(rate.rateMilli)}
            </TableHead>
            {rate.centi.map((centi, i) => (
              <TableCell key={dto.skillsets[i] ?? i} className={cn("text-right", i === OVERALL && "font-semibold")}>
                {format.msd(centi)}
              </TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

function Rated({ dto }: { dto: ChartMsdDto }) {
  const { t } = useTranslation();
  const format = useMsdFormat();
  const [open, setOpen] = useState(false);
  const tableId = useId();
  const overall = dto.rates.find((r) => r.rateMilli === NOMINAL_RATE_MILLI)?.centi[OVERALL];
  return (
    <>
      {overall !== undefined && (
        <p className="flex items-baseline gap-2">
          <span data-testid="msd-overall" className="font-display text-osu-pink text-3xl font-bold tabular-nums">
            {format.msd(overall)}
          </span>
          <span className="text-muted-foreground text-xs">{t("label.msd.overall")}</span>
        </p>
      )}
      {dto.rates.length > 0 && (
        <div className="flex flex-col gap-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-expanded={open}
            aria-controls={open ? tableId : undefined}
            onClick={() => {
              setOpen((o) => !o);
            }}
            className="self-start"
          >
            <ChevronDown aria-hidden className={cn("motion-safe:transition-transform", open && "rotate-180")} />
            {t(open ? "label.msd.hideRates" : "label.msd.showRates")}
          </Button>
          {open && (
            <div id={tableId} className="bg-background/60 rounded-lg border">
              <RateTable dto={dto} />
            </div>
          )}
        </div>
      )}
    </>
  );
}

/** The chart's MinaCalc rating; mounted inside the details dialog so chart_msd runs only once someone looks. */
export function ChartMsd({ md5 }: { md5: string }) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const headingId = useId();
  const msd = useQuery(chartMsdQuery(md5));
  return (
    <section aria-labelledby={headingId} className="bg-muted/30 flex flex-col gap-3 rounded-lg border p-4">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <h3 id={headingId} className="text-sm font-semibold">
          {t("label.msd.title")}
        </h3>
        {msd.data !== undefined && (
          <>
            <Badge className={STATUS_TONE[msd.data.status]}>{t(`label.msd.status.${msd.data.status}`)}</Badge>
            <span className="text-muted-foreground ml-auto text-xs">
              {t("label.msd.calculator", { version: msd.data.calcVersion })}
            </span>
          </>
        )}
      </div>
      {msd.isError ? (
        <p role="alert" className="text-destructive text-sm">
          {errorText(msd.error)}
        </p>
      ) : msd.data === undefined ? (
        <p className="text-muted-foreground text-sm">{t("common.loading")}</p>
      ) : msd.data.status === "rated" ? (
        <Rated dto={msd.data} />
      ) : (
        <StatusNote dto={msd.data} />
      )}
    </section>
  );
}
