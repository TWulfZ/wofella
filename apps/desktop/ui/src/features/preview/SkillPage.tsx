import { useQuery } from "@tanstack/react-query";
import { Radar } from "lucide-react";
import { useTranslation } from "react-i18next";
import { aliasesQuery, useActiveEntry } from "@/features/players";
import { useErrorText } from "@/ipc/errorText";
import { Button } from "@/shared/ui/button";
import { PageHeader } from "@/shared/ui/page-header";
import { BetaBadge } from "./components/BetaBadge";
import { MethodDisclosure } from "./components/MethodDisclosure";
import { ScopePreview } from "./components/ScopePreview";
import { scopeAliasNames } from "./model";
import { skillPreviewQuery } from "./queries";

const NO_ENTRY = { kind: "all_players" } as const;

/** The uncalibrated skill preview (ADR 0024) for the header's scope, keymode and merge mode. */
export function SkillPage() {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const { active, keymode, mergeMode } = useActiveEntry();
  const merge = mergeMode ?? null;
  const previews = useQuery({ ...skillPreviewQuery(active?.ref ?? NO_ENTRY, keymode, merge), enabled: active !== undefined });
  const several = (previews.data?.length ?? 0) > 1;
  const aliases = useQuery({ ...aliasesQuery(), enabled: several });
  const calcVersion = previews.data?.[0]?.calcVersion;

  const names =
    previews.data === undefined || active === undefined || !several
      ? []
      : scopeAliasNames(previews.data, active, aliases.data?.aliases ?? [], mergeMode);
  const scopeName = (index: number, total: number) => {
    const list = names[index];
    return list === undefined
      ? t("preview.scope.unnamed", { n: index + 1, total })
      : list.map((n) => (n === "" ? t("preview.scope.noName") : n)).join(", ");
  };

  return (
    <>
      <PageHeader
        icon={Radar}
        title={t("preview.title")}
        description={t("preview.subtitle")}
        actions={
          <>
            {calcVersion !== undefined && <BetaBadge calcVersion={calcVersion} />}
            <MethodDisclosure />
          </>
        }
      />
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-10 px-6 py-6">
        {previews.isError ? (
          <div role="alert" className="bg-destructive/10 text-destructive flex flex-wrap items-center gap-3 rounded-lg px-3 py-2 text-sm">
            <span>{errorText(previews.error)}</span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                void previews.refetch();
              }}
            >
              {t("common.retry")}
            </Button>
          </div>
        ) : previews.data === undefined ? (
          <p className="text-muted-foreground text-sm">{t("common.loading")}</p>
        ) : (
          previews.data.map((preview, index, all) => (
            <ScopePreview key={preview.scopeHash} preview={preview} name={several ? scopeName(index, all.length) : undefined} />
          ))
        )}
      </div>
    </>
  );
}
