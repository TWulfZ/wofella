import { useTranslation } from "react-i18next";
import { useErrorText } from "@/ipc/errorText";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";

interface QueryAlertProps {
  error: unknown;
  onRetry: () => void;
  /** What the failure costs here, read before the error itself. */
  lead?: string;
  className?: string;
}

export function QueryAlert({ error, onRetry, lead, className }: QueryAlertProps) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  return (
    <div role="alert" className={cn("text-destructive flex flex-wrap items-center gap-3 text-sm", className)}>
      <span>
        {lead !== undefined && `${lead} `}
        {errorText(error)}
      </span>
      <Button variant="outline" size="sm" onClick={onRetry}>
        {t("common.retry")}
      </Button>
    </div>
  );
}
