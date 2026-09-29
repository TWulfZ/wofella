import { useMutation, useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { setupStatusQuery } from "@/features/setup";
import { commands } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { useErrorText } from "@/ipc/errorText";
import { isLanguage, setLanguage, SUPPORTED_LANGUAGES } from "@/shared/i18n";
import { Button } from "@/shared/ui/button";

export const Route = createFileRoute("/settings/")({
  loader: ({ context }) => context.queryClient.query(setupStatusQuery()),
  component: SettingsPage,
});

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[12rem_1fr] items-start gap-4 py-3">
      <dt className="text-muted-foreground text-sm">{label}</dt>
      <dd className="flex flex-col items-start gap-2">{children}</dd>
    </div>
  );
}

function SettingsPage() {
  const { t, i18n } = useTranslation();
  const errorText = useErrorText();
  const { data: status } = useSuspenseQuery(setupStatusQuery());
  const openLogs = useMutation({ mutationFn: () => call(commands.appOpenLogsDir()) });
  return (
    <div className="flex max-w-3xl flex-col gap-4 p-6">
      <h1 className="text-2xl font-semibold">{t("settings.title")}</h1>
      <dl className="divide-y">
        <Row label={t("settings.language")}>
          <div role="radiogroup" aria-label={t("settings.language")} className="flex gap-4">
            {SUPPORTED_LANGUAGES.map((lng) => (
              <label key={lng} className="flex items-center gap-2 text-sm">
                <input
                  type="radio"
                  name="language"
                  value={lng}
                  checked={i18n.resolvedLanguage === lng}
                  onChange={(e) => {
                    if (isLanguage(e.target.value)) {
                      void setLanguage(e.target.value);
                    }
                  }}
                />
                {t(`settings.languages.${lng}`)}
              </label>
            ))}
          </div>
        </Row>
        <Row label={t("settings.dataDir")}>
          <code className="text-sm">{status.dataDir}</code>
        </Row>
        <Row label={t("settings.logsDir")}>
          <code className="text-sm">{status.logsDir}</code>
          <Button
            variant="outline"
            size="sm"
            disabled={openLogs.isPending}
            onClick={() => {
              openLogs.mutate();
            }}
          >
            {t("settings.openLogs")}
          </Button>
          {openLogs.isError && (
            <p role="alert" className="text-destructive text-sm">
              {errorText(openLogs.error)}
            </p>
          )}
        </Row>
        <Row label={t("settings.version")}>
          <span className="text-sm">{status.appVersion}</span>
        </Row>
        <Row label={t("settings.identity")}>
          <Link to="/settings/identity" className="text-primary text-sm underline-offset-4 hover:underline">
            {t("settings.identityLink")}
          </Link>
        </Row>
      </dl>
    </div>
  );
}
