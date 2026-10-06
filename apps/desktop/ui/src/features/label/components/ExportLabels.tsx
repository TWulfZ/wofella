import { useTranslation } from "react-i18next";
import { useErrorText } from "@/ipc/errorText";
import { Button } from "@/shared/ui/button";
import { useLabelExport } from "../queries";

/** The service returns a native path; Windows separators must split too. */
function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function ExportLabels({ keymode }: { keymode: number }) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const exportLabels = useLabelExport(keymode);
  return (
    <span className="flex items-center gap-2">
      {exportLabels.isSuccess && (
        <span role="status" className="text-foreground">
          {t("label.export.done", { count: exportLabels.data.rows, file: fileName(exportLabels.data.path) })}
        </span>
      )}
      {exportLabels.isError && (
        <span role="alert" className="text-destructive">
          {errorText(exportLabels.error)}
        </span>
      )}
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
    </span>
  );
}
