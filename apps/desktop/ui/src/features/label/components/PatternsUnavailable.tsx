import { Link } from "@tanstack/react-router";
import { ArrowRight, Tags } from "lucide-react";
import { useTranslation } from "react-i18next";
import { buttonVariants } from "@/shared/ui/button";
import { Card } from "@/shared/ui/card";
import { PageHeader } from "@/shared/ui/page-header";

export interface PatternsUnavailableProps {
  keymode: number;
  /** Keymodes that do have a pattern taxonomy, offered as a way out. */
  alternatives: readonly number[];
}

export function PatternsUnavailable({ keymode, alternatives }: PatternsUnavailableProps) {
  const { t } = useTranslation();
  return (
    <>
      <PageHeader
        icon={Tags}
        title={t("label.unavailable.title", { keymode })}
        description={t("label.unavailable.body", { keymode })}
      />
      {alternatives.length > 0 && (
        <div className="mx-auto w-full max-w-5xl px-6 py-6">
          <Card className="flex flex-col items-start gap-3 px-5 py-5">
            <p className="text-muted-foreground text-sm">{t("label.unavailable.hint")}</p>
            <div className="flex flex-wrap gap-2">
              {alternatives.map((k) => (
                <Link
                  key={k}
                  to="/label"
                  search={(prev: Record<string, unknown>) => ({ ...prev, keymode: k })}
                  className={buttonVariants({ variant: "outline", size: "sm" })}
                >
                  {t("label.unavailable.switchTo", { keymode: k })}
                  <ArrowRight aria-hidden="true" />
                </Link>
              ))}
            </div>
          </Card>
        </div>
      )}
    </>
  );
}
