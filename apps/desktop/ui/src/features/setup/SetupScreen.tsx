import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type InstallCandidateDto } from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { pickFolder } from "@/ipc/dialog";
import { useErrorText } from "@/ipc/errorText";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { IDENTITY_SETUP_PATH } from "./guard";
import { installCandidatesQuery, setupKeys } from "./queries";

// 002's install scan reports a missing root as this pseudo file name.
const MISSING_DIRECTORY = "directory";

function CandidateDetails({ candidate }: { candidate: InstallCandidateDto }) {
  const { t } = useTranslation();
  const missing = candidate.missing.map((name) => (name === MISSING_DIRECTORY ? t("setup.missingDirectory") : name));
  return (
    <span className="flex flex-wrap items-center gap-2 text-sm">
      <Badge variant="secondary">{t(`setup.source.${candidate.source}`)}</Badge>
      {candidate.osuDbVersion !== null && (
        <span className="text-muted-foreground">{t("setup.osuDbVersion", { version: candidate.osuDbVersion })}</span>
      )}
      {!candidate.valid && <Badge variant="destructive">{t("setup.invalid")}</Badge>}
      {missing.length > 0 && (
        <span className="text-muted-foreground">{t("setup.missing", { files: missing.join(", ") })}</span>
      )}
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

function Choice({ path, checked, disabled = false, onSelect, children }: ChoiceProps) {
  return (
    <li className="rounded-lg border p-3 has-checked:border-primary has-disabled:opacity-60">
      <label className="flex cursor-pointer items-start gap-3">
        <input
          type="radio"
          name="install"
          className="mt-1"
          value={path}
          checked={checked}
          disabled={disabled}
          onChange={() => {
            onSelect(path);
          }}
        />
        <span className="flex flex-col gap-1">
          <code className="text-sm">{path}</code>
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
    <div className="flex max-w-3xl flex-col gap-4 p-6">
      <h1 className="text-2xl font-semibold">{t("setup.title")}</h1>
      <p className="text-muted-foreground">{t("setup.intro")}</p>

      {candidates.isPending && <p>{t("setup.detecting")}</p>}
      {candidates.isError && (
        <p className="text-destructive text-sm">
          {t("setup.detectFailed")} {errorText(candidates.error)}
        </p>
      )}
      {candidates.isSuccess && detected.length === 0 && browsed === null && (
        <p className="text-muted-foreground">{t("setup.noneFound")}</p>
      )}

      <ul aria-label={t("setup.candidates")} className="flex flex-col gap-2">
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

      <div className="flex gap-2">
        <Button variant="outline" onClick={() => void browse()}>
          {t("setup.browse")}
        </Button>
        <Button
          disabled={target === null || confirm.isPending}
          onClick={() => {
            if (target !== null) {
              confirm.mutate(target);
            }
          }}
        >
          {confirm.isPending ? t("setup.confirming") : t("setup.confirm")}
        </Button>
      </div>
    </div>
  );
}
