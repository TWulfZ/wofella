import { FlaskConical } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/shared/ui/badge";

export function BetaBadge({ calcVersion }: { calcVersion: number }) {
  const { t } = useTranslation();
  return (
    <Badge variant="outline" className="border-osu-purple/40 bg-osu-purple/10 text-osu-purple h-6 px-2.5 text-xs">
      <FlaskConical aria-hidden="true" />
      {t("preview.badge", { version: calcVersion })}
    </Badge>
  );
}
