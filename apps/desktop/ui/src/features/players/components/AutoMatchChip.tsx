import { Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AutoMatchDto } from "@/ipc/bindings";
import { Badge } from "@/shared/ui/badge";

// Rendered only from the DTO's autoMatch: the UI never decides who matches (§8, spec 004 Display rules).
export function AutoMatchChip({ autoMatch }: { autoMatch: AutoMatchDto | null }) {
  const { t } = useTranslation();
  if (autoMatch === null) {
    return null;
  }
  return (
    <Badge>
      <Sparkles aria-hidden="true" />
      {t(`players.autoMatch.${autoMatch.source}`)}
    </Badge>
  );
}
