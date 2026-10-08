import { Link } from "@tanstack/react-router";
import { CircleHelp } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { Button, buttonVariants } from "@/shared/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/shared/ui/popover";

const TOPICS = {
  skill: { prefix: "preview.method", paragraphs: ["difficulty", "goal", "rating", "differ"] },
  recs: { prefix: "preview.recs.method", paragraphs: ["band", "dominant", "sets", "rates", "differ"] },
} as const;

export function MethodDisclosure({ topic = "skill" }: { topic?: keyof typeof TOPICS }) {
  const { t } = useTranslation();
  const { prefix, paragraphs } = TOPICS[topic];
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button type="button" variant="outline" size="sm">
          <CircleHelp aria-hidden="true" />
          {t("preview.method.trigger")}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" collisionPadding={12} className="flex w-96 max-w-[calc(100vw-2rem)] flex-col gap-3 text-sm leading-relaxed">
        <p className="font-display text-base font-semibold">{t(`${prefix}.title`)}</p>
        {paragraphs.map((key) => (
          <p key={key} className={cn(key === "differ" && "text-muted-foreground")}>
            {t(`${prefix}.${key}`)}
          </p>
        ))}
        <div className="flex flex-col items-start gap-2 border-t pt-3">
          <p className="text-muted-foreground text-xs">{t("preview.method.calibrate")}</p>
          <Link to="/label" className={buttonVariants({ variant: "outline", size: "sm" })}>
            {t("preview.method.labelLink")}
          </Link>
        </div>
      </PopoverContent>
    </Popover>
  );
}
