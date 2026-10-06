import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { ArrowRight, CircleCheck, FolderSearch, LoaderCircle, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "cn";
import { commands, type InstallCandidateDto } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { pickFolder } from "@/ipc/dialog";
import { useErrorText } from "@/ipc/errorText";
import { openJobTray } from "@/features/jobs";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { PageHeader } from "@/shared/ui/page-header";
import { IDENTITY_SETUP_PATH } from "./guard";
import { installCandidatesQuery, setupKeys } from "./queries";
import { SetupSteps } from "./SetupSteps";

// 002's install scan reports a missing root as this pseudo file name.
const MISSING_DIRECTORY = "directory";

function CandidateDetails({ candidate }: { candidate: InstallCandidateDto }) {
  const { t } = useTranslation();
  const missing = candidate.missing.map((name) => (name === MISSING_DIRECTORY ? t("setup.missingDirectory") : name));
  const StatusIcon = candidate.valid ? CircleCheck : TriangleAlert;
  return (
    <span className="flex flex-col gap-2 text-sm">
      <span className={cn("flex items-center gap-1.5 font-medium", candidate.valid ? "text-success" : "text-warning")}>
        <StatusIcon className="size-4" aria-hidden="true" />
        {candidate.valid ? t("setup.valid") : t("setup.invalid")}
      </span>
      <span className="flex flex-wrap items-center gap-2">
        <Badge variant="secondary">{t(`setup.source.${candidate.source}`)}</Badge>
        {candidate.osuDbVersion !== null && (
          <Badge variant="outline" className="tabular">
            {t("setup.osuDbVersion", { version: candidate.osuDbVersion })}
          </Badge>
        )}
        {missing.length > 0 && (
          <span className="text-muted-foreground">{t("setup.missing", { files: missing.join(", ") })}</span>
        )}
      </span>
    </span>
  );
}

interface ChoiceProps {
  path: string;
  checked: boolean;
  disabled?: boolean;
  onSelect: (path: string) => void;
  children: React.ReactNode;
}

// The native radio stays in the tree (visually hidden) so arrow keys, the form group and its label keep working.
function Choice({ path, checked, disabled = false, onSelect, children }: ChoiceProps) {
  return (
    <li className="bg-card ring-border has-checked:ring-primary has-checked:bg-primary/5 has-focus-visible:outline-ring has-enabled:hover:bg-surface-raised rounded-xl ring-1 transition-colors duration-200 has-checked:ring-2 has-disabled:opacity-75 has-focus-visible:outline-2 has-focus-visible:outline-offset-2">
      <label className="flex cursor-pointer items-start gap-3 p-4 has-disabled:cursor-not-allowed">
        <input
          type="radio"
          name="install"
          className="peer sr-only"
          value={path}
          checked={checked}
          disabled={disabled}
          onChange={() => {
            onSelect(path);
          }}
        />
        <span
          aria-hidden="true"
          className="ring-control-border peer-checked:ring-primary mt-1.5 grid size-4 shrink-0 place-items-center rounded-full ring-2 transition-colors duration-200 peer-checked:*:bg-primary"
        >
          <span className="size-2 rounded-full" />
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-2">
          <code className="bg-muted self-start rounded-md px-3 py-1.5 font-mono text-sm break-all">{path}</code>
          {children}
        </span>
      </label>
    </li>
  );
}

export function SetupScreen() {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const candidates = useQuery(installCandidatesQuery());
  const [browsed, setBrowsed] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);

  const detected = candidates.data ?? [];
  const firstValid = detected.find((c) => c.valid)?.path ?? null;
  const target = selected ?? browsed ?? firstValid;

  const confirm = useMutation({
    mutationFn: async (path: string) => {
      const install = await call(commands.setupSetInstallPath(path));
      await call(commands.jobsStart({ kind: "sync_plays", installId: install.id }));
      return install;
    },
    onSuccess: async () => {
      // The root guard re-reads setup status on navigation; it must not route on the pre-install cached value.
      // Only status: re-running detection would spawn reg.exe and a drive scan again for nothing.
      await queryClient.invalidateQueries({ queryKey: setupKeys.status() });
      openJobTray();
      await navigate({ to: IDENTITY_SETUP_PATH });
    },
  });

  const browse = async () => {
    const folder = await pickFolder(target ?? undefined);
    if (folder !== null) {
      setBrowsed(folder);
      setSelected(folder);
    }
  };

  return (
    <>
      {/* Same shell as the identity step, so the stepper and content edges stay put across the flow. */}
      <PageHeader
        icon={FolderSearch}
        title={t("setup.title")}
        description={t("setup.intro")}
        actions={<SetupSteps current="install" />}
      />

      <div className="mx-auto flex w-full max-w-5xl flex-col gap-4 px-6 py-6">
        {candidates.isPending && (
          <p className="text-muted-foreground flex items-center gap-2">
            <LoaderCircle className="text-osu-blue size-4 motion-safe:animate-spin" aria-hidden="true" />
            {t("setup.detecting")}
          </p>
        )}
        {candidates.isError && (
          <p className="text-destructive text-sm">
            {t("setup.detectFailed")} {errorText(candidates.error)}
          </p>
        )}
        {candidates.isSuccess && detected.length === 0 && browsed === null && (
          <p className="text-muted-foreground">{t("setup.noneFound")}</p>
        )}

        <ul aria-label={t("setup.candidates")} className="flex flex-col gap-3">
          {detected.map((candidate) => (
            <Choice
              key={candidate.path}
              path={candidate.path}
              checked={target === candidate.path}
              disabled={!candidate.valid}
              onSelect={setSelected}
            >
              <CandidateDetails candidate={candidate} />
            </Choice>
          ))}
          {browsed !== null && !detected.some((c) => c.path === browsed) && (
            <Choice path={browsed} checked={target === browsed} onSelect={setSelected}>
              <span className="text-muted-foreground text-sm">{t("setup.customPath")}</span>
            </Choice>
          )}
        </ul>

        {confirm.isError && (
          <p role="alert" className="text-destructive text-sm">
            {errorText(confirm.error)}
          </p>
        )}

        <div className="flex flex-wrap items-center justify-between gap-2">
          <Button variant="outline" size="lg" onClick={() => void browse()}>
            <FolderSearch aria-hidden="true" />
            {t("setup.browse")}
          </Button>
          <Button
            size="lg"
            disabled={target === null || confirm.isPending}
            onClick={() => {
              if (target !== null) {
                confirm.mutate(target);
              }
            }}
          >
            {confirm.isPending ? t("setup.confirming") : t("setup.confirm")}
            <ArrowRight aria-hidden="true" />
          </Button>
        </div>
      </div>
    </>
  );
}
