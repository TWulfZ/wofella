import { useTranslation } from "react-i18next";
import { Checkbox } from "@/shared/ui/checkbox";

interface SelectAllCheckboxProps {
  ticked: number;
  total: number;
  onChange: (tickAll: boolean) => void;
}

export function SelectAllCheckbox({ ticked, total, onChange }: SelectAllCheckboxProps) {
  const { t } = useTranslation();
  const state = ticked === 0 ? false : ticked === total ? true : "indeterminate";
  return (
    <Checkbox
      aria-label={t("players.table.selectAll")}
      checked={state}
      disabled={total === 0}
      onCheckedChange={() => {
        onChange(state !== true);
      }}
    />
  );
}
