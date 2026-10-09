import { Link } from "@tanstack/react-router";
import { Info, TriangleAlert } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { buttonVariants } from "@/shared/ui/button";

// 7K ratings carry the stronger warning and the way to help fix them (ADR 0024 Both keymodes).
const STRONG_WARNING = "k7_less_validated";

export function WarningsBanner({ warnings }: { warnings: readonly string[] }) {
  const { t, i18n } = useTranslation();
  const titleId = useId();
  if (warnings.length === 0) {
    return null;
  }
  const strong = warnings.includes(STRONG_WARNING);
  const Icon = strong ? TriangleAlert : Info;
  const line = (code: string) => {
    const key = `preview.warnings.code.${code}`;
    return i18n.exists(key) ? t(key) : t("preview.warnings.unknown", { code });
  };
  return (
    <div
      className={cn(
        "flex flex-col gap-3 rounded-xl border p-4 sm:flex-row sm:items-start",
        strong ? "border-warning/40 bg-warning/10" : "bg-muted/40",
      )}
    >
      <Icon aria-hidden="true" className={cn("mt-0.5 size-4 shrink-0", strong ? "text-warning" : "text-muted-foreground")} />
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <span id={titleId} className="text-sm font-semibold">
          {t("preview.warnings.title")}
        </span>
        <ul aria-labelledby={titleId} className="text-muted-foreground flex flex-col gap-1 text-sm">
          {warnings.map((code) => (
            <li key={code}>{line(code)}</li>
          ))}
        </ul>
      </div>
      {strong && (
        <Link to="/label" className={cn(buttonVariants({ variant: "outline", size: "sm" }), "shrink-0 self-start")}>
          {t("preview.warnings.cta")}
        </Link>
      )}
    </div>
  );
}
