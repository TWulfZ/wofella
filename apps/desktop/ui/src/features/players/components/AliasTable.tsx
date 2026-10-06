import { useTranslation } from "react-i18next";
import type { AliasRowDto } from "@/ipc/bindings";
import { Table, TableBody, TableHead, TableHeader, TableRow } from "@/shared/ui/table";
import { AliasRow, type AliasTableMode } from "./AliasRow";
import { SelectAllCheckbox } from "./SelectAllCheckbox";

interface AliasTableProps {
  rows: readonly AliasRowDto[];
  ticked: ReadonlySet<number>;
  mode: AliasTableMode;
  onToggle: (aliasId: number) => void;
  onToggleAll: (tickAll: boolean) => void;
}

// Rows arrive in the backend's R6 order and are rendered as-is (spec 004 IPC / UI).
export function AliasTable({ rows, ticked, mode, onToggle, onToggleAll }: AliasTableProps) {
  const { t } = useTranslation();
  return (
    <Table>
      <TableHeader className="bg-header/50">
        <TableRow className="hover:bg-transparent">
          <TableHead className="w-8 pl-4">
            <SelectAllCheckbox ticked={ticked.size} total={rows.length} onChange={onToggleAll} />
          </TableHead>
          <TableHead>{t("players.table.name")}</TableHead>
          <TableHead className="text-right">{t("players.table.plays")}</TableHead>
          <TableHead>{t("players.table.keymodes")}</TableHead>
          <TableHead>{t("players.table.played")}</TableHead>
          <TableHead>{t("players.table.split")}</TableHead>
          <TableHead className="text-right">{t("players.table.replays")}</TableHead>
          <TableHead className="pr-4">{t("players.table.topCharts")}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((row) => (
          <AliasRow key={row.aliasId} row={row} checked={ticked.has(row.aliasId)} mode={mode} onToggle={onToggle} />
        ))}
      </TableBody>
    </Table>
  );
}
