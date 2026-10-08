import { useMutation, useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Settings as SettingsIcon, UserRound } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { labelPatternExamplesQuery, skinListQuery, useSkinFile } from "@/features/label";
import {
  DefaultSkinCard,
  HandLayoutCard,
  previewExample,
  readSkinChoice,
  selectedSkinFolder,
  SessionNotifyCard,
  skinOptions,
  writeSkinChoice,
} from "@/features/preferences";
import { DEFAULT_KEYMODE } from "@/features/players";
import { setupStatusQuery } from "@/features/setup";
import { commands } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { useErrorText } from "@/ipc/errorText";
import { isLanguage, setLanguage, SUPPORTED_LANGUAGES } from "@/shared/i18n";
import { Button, buttonVariants } from "@/shared/ui/button";
import { Card, CardContent } from "@/shared/ui/card";
import { PageHeader } from "@/shared/ui/page-header";

export const Route = createFileRoute("/settings/")({
  loader: ({ context }) => context.queryClient.query(setupStatusQuery()),
  component: SettingsPage,
});

const PATH_CHIP = "bg-muted self-stretch rounded-md px-3 py-2 font-mono text-sm break-all";

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[12rem_1fr] items-baseline gap-4 py-4">
      <dt className="text-muted-foreground text-sm">{label}</dt>
      <dd className="flex flex-col items-start gap-2">{children}</dd>
    </div>
  );
}

function DefaultSkinSection({ keymode }: { keymode: number }) {
  const skinList = useQuery(skinListQuery());
  const [choice, setChoice] = useState(readSkinChoice);
  const folder = selectedSkinFolder(skinList.data, choice);
  const skinFile = useSkinFile(folder, keymode, skinList.data);
  // Null draws with the stored hand layout, as the Label screen's pattern previews do.
  const examples = useQuery(labelPatternExamplesQuery(keymode, null));
  return (
    <DefaultSkinCard
      options={skinOptions(skinList.data, keymode)}
      folder={folder}
      ready={skinList.data !== undefined}
      skin={skinFile.data?.dto ?? null}
      example={examples.isError ? null : examples.data === undefined ? undefined : (previewExample(examples.data) ?? null)}
      error={skinList.error ?? skinFile.error ?? examples.error}
      onChange={(next) => {
        setChoice({ folder: next });
        writeSkinChoice({ folder: next });
      }}
    />
  );
}

function SettingsPage() {
  const { t, i18n } = useTranslation();
  const errorText = useErrorText();
  const { data: status } = useSuspenseQuery(setupStatusQuery());
  const keymode = Route.useSearch({ select: (search) => search.keymode ?? DEFAULT_KEYMODE });
  const openLogs = useMutation({ mutationFn: () => call(commands.appOpenLogsDir()) });
  return (
    <>
      <PageHeader icon={SettingsIcon} title={t("settings.title")} description={t("settings.subtitle")} />
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-6 px-6 py-6">
        <Card className="py-0">
          <CardContent>
            <dl className="divide-y">
              <Row label={t("settings.language")}>
                <div
                  role="radiogroup"
                  aria-label={t("settings.language")}
                  className="bg-surface-raised inline-flex gap-1 rounded-lg p-1"
                >
                  {SUPPORTED_LANGUAGES.map((lng) => (
                    <label
                      key={lng}
                      className="bg-muted text-muted-foreground hover:text-foreground has-[:checked]:bg-primary has-[:checked]:text-primary-foreground has-[:focus-visible]:ring-ring has-[:focus-visible]:ring-offset-2 has-[:focus-visible]:ring-offset-surface-raised cursor-pointer rounded-md px-3 py-1.5 text-sm transition-colors has-[:focus-visible]:ring-2"
                    >
                      <input
                        type="radio"
                        name="language"
                        value={lng}
                        className="sr-only"
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
                <code className={PATH_CHIP}>{status.dataDir}</code>
              </Row>
              <Row label={t("settings.logsDir")}>
                <code className={PATH_CHIP}>{status.logsDir}</code>
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
                <span className="tabular text-sm">{status.appVersion}</span>
              </Row>
              <Row label={t("settings.identity")}>
                <Link to="/settings/identity" className={buttonVariants({ variant: "outline", size: "sm" })}>
                  <UserRound aria-hidden="true" />
                  {t("settings.identityLink")}
                </Link>
              </Row>
            </dl>
          </CardContent>
        </Card>
        <SessionNotifyCard />
        <HandLayoutCard keymode={keymode} />
        <DefaultSkinSection keymode={keymode} />
      </div>
    </>
  );
}
