import { useTranslation } from "react-i18next";
import { useErrorText } from "@/ipc/errorText";
import { Button } from "@/shared/ui/button";
import type { LabelExport } from "../queries";

/** The service returns a native path; Windows separators must split too. */
function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** The mutation is the session's, so its outcome survives the settings flyout closing over it. */
export function ExportLabels({ exportLabels }: { exportLabels: LabelExport }) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  return (
    <div className="flex min-w-0 flex-col items-start gap-1.5 text-xs">
      <Button
        size="sm"
        variant="outline"
        disabled={exportLabels.isPending}
        onClick={() => {
          exportLabels.mutate();
        }}
      >
        {t("label.export.button")}
      </Button>
      {exportLabels.isSuccess && (
        <p role="status" className="text-foreground break-words">
          {t("label.export.done", { count: exportLabels.data.rows, file: fileName(exportLabels.data.path) })}
        </p>
      )}
      {exportLabels.isError && (
        <p role="alert" className="text-destructive break-words">
          {errorText(exportLabels.error)}
        </p>
      )}
    </div>
  );
}
