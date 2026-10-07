import { Link } from "@tanstack/react-router";
import { ArrowRight, Hourglass, type LucideIcon, Sparkle } from "lucide-react";
import type { ReactNode } from "react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/shared/ui/badge";
import { buttonVariants } from "@/shared/ui/button";
import { Card } from "@/shared/ui/card";
import { PageHeader } from "@/shared/ui/page-header";

export function ComingSoonPage({
  icon,
  title,
  subtitle,
  preview,
  features,
}: {
  icon: LucideIcon;
  title: string;
  subtitle: string;
  preview: ReactNode;
  features: readonly string[];
}) {
  const { t } = useTranslation();
  const featuresId = useId();
  return (
    <>
      <PageHeader
        icon={icon}
        title={title}
        description={subtitle}
        actions={
          <Badge variant="outline" className="border-osu-yellow/40 bg-osu-yellow/10 text-osu-yellow h-6 px-2.5 text-xs">
            <Hourglass aria-hidden="true" />
            {t("roadmap.comingSoon")}
          </Badge>
        }
      />
      <div className="mx-auto grid w-full max-w-5xl gap-6 px-6 py-6 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <Card className="osu-triangles relative gap-3 px-5 py-5">
          <figure className="flex flex-col gap-3">
            {preview}
            <figcaption className="text-muted-foreground text-xs">{t("roadmap.previewCaption")}</figcaption>
          </figure>
        </Card>
        <section aria-labelledby={featuresId} className="flex flex-col gap-4">
          <h2 id={featuresId} className="font-display text-lg font-semibold">
            {t("roadmap.whatItWillDo")}
          </h2>
          <ul className="flex flex-col gap-3">
            {features.map((feature) => (
              <li key={feature} className="flex gap-3 text-sm leading-relaxed">
                <Sparkle className="text-primary mt-1 size-3.5 shrink-0" aria-hidden="true" />
                <span>{feature}</span>
              </li>
            ))}
          </ul>
          <div className="mt-auto flex flex-col items-start gap-2 border-t pt-4">
            <Link to="/label" className={buttonVariants({ variant: "outline", size: "sm" })}>
              {t("roadmap.meanwhile")}
              <ArrowRight aria-hidden="true" />
            </Link>
            <p className="text-muted-foreground text-xs">{t("roadmap.meanwhileHint")}</p>
          </div>
        </section>
      </div>
    </>
  );
}
