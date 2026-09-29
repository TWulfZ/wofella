import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { openJobTray, useRunningJobKinds } from "@/features/jobs";
import { setupKeys, setupStatusQuery } from "@/features/setup";
import { commands } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { useErrorText } from "@/ipc/errorText";
import { Button } from "@/shared/ui/button";
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

  return (
    <div className="flex flex-col gap-4 p-6">
      <h1 className="text-2xl font-semibold">{title}</h1>
      <p className="text-muted-foreground max-w-3xl">{intro}</p>

      {aliases.isPending && (
        <div aria-busy="true" aria-label={t("common.loading")} className="flex flex-col gap-2">
          {Array.from({ length: SKELETON_ROWS }, (_, i) => (
            <div key={i} className="bg-muted h-8 animate-pulse rounded" />
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

      {syncing && <p className="text-sm">{t("players.wizard.syncing")}</p>}

      {aliases.isSuccess && rows.length === 0 && !syncing && (
        <div className="flex flex-col items-start gap-2">
          <p>{t("players.wizard.empty")}</p>
          <SyncNowButton />
        </div>
      )}

      {rows.length > 0 && (
        <>
          {aliases.data?.cfgUsernameAvailable === false && (
            <p className="text-muted-foreground text-sm">{t("players.wizard.cfgMissing")}</p>
          )}
          {noAutoMatch && edited === null && <p className="font-medium">{t("players.wizard.noMatch")}</p>}
          <AliasTable rows={rows} ticked={ticked} mode={mode} onToggle={toggle} onToggleAll={toggleAll} />
          {decide.isError && (
            <p role="alert" className="text-destructive text-sm">
              {errorText(decide.error)}
            </p>
          )}
          <div>
            <Button
              disabled={syncing || decide.isPending}
              onClick={() => {
                decide.mutate();
              }}
            >
              {decide.isPending
                ? t("players.wizard.submitting")
                : mode === "wizard"
                  ? t("players.wizard.confirm")
                  : t("players.settings.save")}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
