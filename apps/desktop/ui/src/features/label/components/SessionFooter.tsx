import { useTranslation } from "react-i18next";
import type { SessionCounts } from "../session";

interface SessionFooterProps {
  counts: SessionCounts;
  goldTotal: number | null;
  seed: string;
}

export function SessionFooter({ counts, goldTotal, seed }: SessionFooterProps) {
  const { t } = useTranslation();
  return (
    <footer className="text-muted-foreground flex flex-wrap items-center gap-x-4 gap-y-1 border-t pt-2 text-xs">
      <span>{t("label.footer.labelled", { count: counts.labelled })}</span>
      <span>{t("label.footer.skipped", { count: counts.skipped })}</span>
      <span>{t("label.footer.undone", { count: counts.undone })}</span>
      {goldTotal !== null && (
        <span className="text-foreground font-medium">{t("label.footer.goldTotal", { count: goldTotal })}</span>
      )}
      <span className="ml-auto font-mono">{t("label.footer.seed", { seed })}</span>
    </footer>
  );
}
