import { useTranslation } from "react-i18next";
import type { AliasRowDto, KeymodeCountDto } from "@/ipc/bindings";
import { formatNumber } from "@/shared/format";
import { Badge } from "@/shared/ui/badge";
import { Checkbox } from "@/shared/ui/checkbox";
import { TableCell, TableRow } from "@/shared/ui/table";
import { AutoMatchChip } from "./AutoMatchChip";

// The MVP keymode leads the per-keymode counts (spec 004 Behaviour 2); the rest keep the DTO order.
const PRIMARY_BUCKET = "k7";
const COLUMN_BUCKET = /^k(\d+)$/;
const SHORT_MD5 = 8;

export type AliasTableMode = "wizard" | "settings";

function useKeymodeLabel(): (bucket: string) => string {
  const { t } = useTranslation();
  return (bucket) => {
    const columns = COLUMN_BUCKET.exec(bucket)?.[1];
    return columns === undefined ? t(`players.keymode.${bucket}`) : `${columns}K`;
  };
}

function primaryFirst(counts: readonly KeymodeCountDto[]): KeymodeCountDto[] {
  return [...counts.filter((c) => c.bucket === PRIMARY_BUCKET), ...counts.filter((c) => c.bucket !== PRIMARY_BUCKET)];
}

export function AliasName({ row }: { row: AliasRowDto }) {
  const { t } = useTranslation();
  if (row.isEmptyName) {
    return (
      <span data-testid="alias-name">
        <span className="italic">{t("players.noName")}</span>
        <code className="text-muted-foreground ml-1 text-xs">""</code>
      </span>
    );
  }
  return <span data-testid="alias-name">{row.rawName}</span>;
}

interface AliasRowProps {
  row: AliasRowDto;
  checked: boolean;
  mode: AliasTableMode;
  onToggle: (aliasId: number) => void;
}

export function AliasRow({ row, checked, mode, onToggle }: AliasRowProps) {
  const { t, i18n } = useTranslation();
  const keymodeLabel = useKeymodeLabel();
  const n = (value: number) => formatNumber(value, i18n.language);
  const day = (iso: string) => new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" }).format(new Date(iso));
  return (
    <TableRow data-state={checked ? "selected" : undefined}>
      <TableCell>
        <Checkbox
          aria-label={row.isEmptyName ? t("players.noName") : row.rawName}
          checked={checked}
          onCheckedChange={() => {
            onToggle(row.aliasId);
          }}
        />
      </TableCell>
      <TableCell>
        <div className="flex flex-wrap items-center gap-2">
          <AliasName row={row} />
          <AutoMatchChip autoMatch={row.autoMatch} />
          {mode === "settings" && row.decision !== null && (
            <Badge variant={row.decision === "me" ? "default" : "outline"}>{t(`players.decision.${row.decision}`)}</Badge>
          )}
        </div>
      </TableCell>
      <TableCell className="tabular-nums">{n(row.nPlays)}</TableCell>
      <TableCell className="text-xs">
        {primaryFirst(row.byKeymode)
          .map((c) => `${keymodeLabel(c.bucket)} ${n(c.n)}`)
          .join(" · ")}
      </TableCell>
      <TableCell className="text-xs">
        {row.firstPlayedAt !== null &&
          row.lastPlayedAt !== null &&
          t("players.dateRange", { from: day(row.firstPlayedAt), to: day(row.lastPlayedAt) })}
      </TableCell>
      <TableCell className="text-xs">{t("players.split", { online: n(row.nOnline), offline: n(row.nOffline) })}</TableCell>
      <TableCell className="tabular-nums">{n(row.nWithReplay)}</TableCell>
      <TableCell className="text-xs">
        <ul>
          {row.topCharts.map((chart) => (
            <li key={chart.chartMd5}>
              {chart.title ?? <code>{chart.chartMd5.slice(0, SHORT_MD5)}</code>}
              {chart.version !== null && <span className="text-muted-foreground"> [{chart.version}]</span>}{" "}
              <span className="text-muted-foreground">{t("players.chartTimes", { n: n(chart.n) })}</span>
            </li>
          ))}
        </ul>
      </TableCell>
    </TableRow>
  );
}
