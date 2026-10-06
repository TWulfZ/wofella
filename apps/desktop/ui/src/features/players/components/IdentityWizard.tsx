import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { Check, Inbox, LoaderCircle, UsersRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { openJobTray, useRunningJobKinds } from "@/features/jobs";
import { SetupSteps, setupKeys, setupStatusQuery } from "@/features/setup";
import { commands } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { useErrorText } from "@/ipc/errorText";
import { Button } from "@/shared/ui/button";
import { Card, CardContent } from "@/shared/ui/card";
import { PageHeader } from "@/shared/ui/page-header";
import { buildDecisions } from "../decisions";
import { aliasesQuery, playersKeys } from "../queries";
import type { AliasTableMode } from "./AliasRow";
import { AliasTable } from "./AliasTable";

const SKELETON_ROWS = 4;

function SyncNowButton() {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const status = useQuery(setupStatusQuery());
  const install = status.data?.install ?? null;
  const sync = useMutation({
    mutationFn: (installId: number) => call(commands.jobsStart({ kind: "sync_plays", installId })),
    onSuccess: openJobTray,
  });
  if (install === null) {
    return null;
  }
  return (
    <div className="flex flex-col items-start gap-2">
      <Button
        disabled={sync.isPending}
        onClick={() => {
          sync.mutate(install.id);
        }}
      >
        {t("players.wizard.syncNow")}
      </Button>
      {sync.isError && (
        <p role="alert" className="text-destructive text-sm">
          {errorText(sync.error)}
        </p>
      )}
    </div>
  );
}

interface IdentityWizardProps {
  mode: AliasTableMode;
}

/** "Which of these are you?" (mode wizard) and Settings → Identity (mode settings) share one table and flow. */
export function IdentityWizard({ mode }: IdentityWizardProps) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const aliases = useQuery(aliasesQuery());
  const runningKinds = useRunningJobKinds();
  // Null until the user touches a box: until then the table follows the backend selection as it refreshes.
  const [edited, setEdited] = useState<ReadonlySet<number> | null>(null);

  const rows = aliases.data?.aliases ?? [];
  const ticked = edited ?? new Set(rows.filter((r) => r.selected).map((r) => r.aliasId));
  // Spec 004 Wizard states: the list is still changing while a sync or its identity refresh runs.
  const syncing = runningKinds.has("sync_plays") || runningKinds.has("refresh_identity");

  const decide = useMutation({
    mutationFn: () =>
      call(commands.playersDecideAlias({ decisions: buildDecisions(rows, ticked), completesWizard: mode === "wizard" })),
    onSuccess: async (list) => {
      queryClient.setQueryData(playersKeys.aliases(), list);
      setEdited(null);
      // identityReady changes with the wizard; the root guard must not bounce back here on a stale status.
      await queryClient.invalidateQueries({ queryKey: setupKeys.status() });
      if (mode === "wizard") {
        await navigate({ to: "/" });
      } else {
        toast.success(t("players.settings.saved"));
      }
    },
  });

  const toggle = (aliasId: number) => {
    const next = new Set(ticked);
    if (!next.delete(aliasId)) {
      next.add(aliasId);
    }
    setEdited(next);
  };

  const toggleAll = (tickAll: boolean) => {
    setEdited(new Set(tickAll ? rows.map((r) => r.aliasId) : []));
  };

  const title = mode === "wizard" ? t("players.wizard.title") : t("players.settings.title");
  const intro = mode === "wizard" ? t("players.wizard.intro") : t("players.settings.intro");
  const noAutoMatch = rows.length > 0 && rows.every((r) => r.autoMatch === null);

  const actionsLabel = mode === "wizard" ? t("players.wizard.actions") : t("players.settings.actions");

  return (
    <>
      <PageHeader
        icon={UsersRound}
        title={title}
        description={intro}
        actions={mode === "wizard" ? <SetupSteps current="identity" /> : undefined}
      />
      <div className="mx-auto flex w-full max-w-5xl flex-1 flex-col gap-4 px-6 py-6">
        {aliases.isPending && (
          <div aria-busy="true" aria-label={t("common.loading")} className="flex flex-col gap-2">
            {Array.from({ length: SKELETON_ROWS }, (_, i) => (
              <div key={i} className="bg-muted h-10 rounded-lg motion-safe:animate-pulse" />
            ))}
          </div>
        )}

        {aliases.isError && (
          <div className="flex flex-col items-start gap-2">
            <p role="alert" className="text-destructive">
              {errorText(aliases.error)}
            </p>
            <Button variant="outline" onClick={() => void aliases.refetch()}>
              {t("common.retry")}
            </Button>
          </div>
        )}

        {syncing && (
          <p className="bg-osu-blue/10 ring-osu-blue/30 flex items-center gap-2 rounded-lg px-4 py-3 text-sm ring-1">
            <LoaderCircle className="text-osu-blue size-4 shrink-0 motion-safe:animate-spin" aria-hidden="true" />
            {t("players.wizard.syncing")}
          </p>
        )}

        {aliases.isSuccess && rows.length === 0 && !syncing && (
          <Card>
            <CardContent className="flex flex-col items-center gap-3 py-6 text-center">
              <Inbox className="text-muted-foreground size-8" aria-hidden="true" />
              <p>{t("players.wizard.empty")}</p>
              <SyncNowButton />
            </CardContent>
          </Card>
        )}

        {rows.length > 0 && (
          <>
            {aliases.data?.cfgUsernameAvailable === false && (
              <p className="text-muted-foreground text-sm">{t("players.wizard.cfgMissing")}</p>
            )}
            {noAutoMatch && edited === null && <p className="font-medium">{t("players.wizard.noMatch")}</p>}
            <Card className="gap-0 py-0">
              <AliasTable rows={rows} ticked={ticked} mode={mode} onToggle={toggle} onToggleAll={toggleAll} />
            </Card>
          </>
        )}
      </div>

      {rows.length > 0 && (
        <section aria-label={actionsLabel} className="bg-header/90 sticky bottom-0 z-10 border-t backdrop-blur">
          {/* Actions sit on the left: the floating job tray owns the bottom-right corner. */}
          <div className="mx-auto flex w-full max-w-5xl flex-wrap items-center gap-4 px-6 py-3">
            <Button
              size="lg"
              disabled={syncing || decide.isPending}
              onClick={() => {
                decide.mutate();
              }}
            >
              <Check aria-hidden="true" />
              {decide.isPending
                ? t("players.wizard.submitting")
                : mode === "wizard"
                  ? t("players.wizard.confirm")
                  : t("players.settings.save")}
            </Button>
            <span className="text-muted-foreground tabular text-sm">{t("players.selected", { count: ticked.size })}</span>
            {decide.isError && (
              <p role="alert" className="text-destructive text-sm">
                {errorText(decide.error)}
              </p>
            )}
          </div>
        </section>
      )}
    </>
  );
}
