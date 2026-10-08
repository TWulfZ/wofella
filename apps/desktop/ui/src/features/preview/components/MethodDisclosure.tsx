import { Link } from "@tanstack/react-router";
import { CircleHelp } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/shared/lib/utils";
import { Button, buttonVariants } from "@/shared/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/shared/ui/popover";

const PARAGRAPHS = ["difficulty", "goal", "rating", "differ"] as const;

export function MethodDisclosure() {
  const { t } = useTranslation();
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button type="button" variant="outline" size="sm">
          <CircleHelp aria-hidden="true" />
          {t("preview.method.trigger")}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" collisionPadding={12} className="flex w-96 max-w-[calc(100vw-2rem)] flex-col gap-3 text-sm leading-relaxed">
        <p className="font-display text-base font-semibold">{t("preview.method.title")}</p>
        {PARAGRAPHS.map((key) => (
          <p key={key} className={cn(key === "differ" && "text-muted-foreground")}>
            {t(`preview.method.${key}`)}
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
