import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Compass, Gamepad2, LoaderCircle, SearchX } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { useActiveEntry } from "@/features/players";
import type { EntryRefDto, RecItemDto, RecsModeDto, RecsPreviewDto } from "@/ipc/bindings";
import { useErrorText } from "@/ipc/errorText";
import { cn } from "@/shared/lib/utils";
import { Button, buttonVariants } from "@/shared/ui/button";
import { PageHeader } from "@/shared/ui/page-header";
import { BetaBadge } from "./components/BetaBadge";
import { MethodDisclosure } from "./components/MethodDisclosure";
import { RecCard } from "./components/RecCard";
import { ModeTabs, SkillsetPicker } from "./components/RecsControls";
import { WarningsBanner } from "./components/WarningsBanner";
import { usePreviewFormat } from "./format";
import { pickableSkillsets } from "./model";
import { recsPreviewQuery } from "./queries";

const NO_ENTRY = { kind: "all_players" } as const;
const CARD = "bg-card ring-border flex flex-col gap-4 rounded-xl p-5 shadow-lg shadow-black/20 ring-1";

function sameView(key: readonly unknown[] | undefined, entry: EntryRefDto, keymode: number): boolean {
  return key !== undefined && JSON.stringify(key[2]) === JSON.stringify(entry) && key[3] === keymode;
}

function Summary({ recs }: { recs: RecsPreviewDto }) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  const [lo, hi] = recs.bandCenti;
  return (
    <div
      data-testid="recs-summary"
      className="bg-card ring-border osu-triangles flex flex-wrap items-center gap-x-6 gap-y-2 rounded-xl px-5 py-4 ring-1"
    >
      <span className="font-display text-lg font-bold">{t("preview.recs.summary.focus", { skillset: format.skillset(recs.focus) })}</span>
      <span className="flex items-center gap-2 text-sm tabular-nums">
        <span aria-hidden="true" className="bg-foreground/50 h-3.5 w-0.5 rounded-full" />
        {t("preview.recs.summary.rating", { value: format.approx(recs.ratingCenti) })}
      </span>
      <span className="flex items-center gap-2 text-sm tabular-nums">
        <span aria-hidden="true" className="bg-osu-pink size-2.5 rounded-full" />
        {t("preview.recs.summary.band", { lo: format.approx(lo), hi: format.plain(hi) })}
      </span>
    </div>
  );
}

function Computing() {
  const { t } = useTranslation();
  return (
    <div role="status" className={`${CARD} items-center py-10 text-center`}>
      <LoaderCircle aria-hidden="true" className="text-primary size-8 motion-safe:animate-spin" />
      <p className="font-display text-lg font-semibold">{t("preview.state.computing")}</p>
      <p className="text-muted-foreground max-w-md text-sm">{t("preview.state.computingHint")}</p>
    </div>
  );
}

function NoRating({ keymode }: { keymode: number }) {
  const { t } = useTranslation();
  return (
    <div className={`${CARD} items-center py-10 text-center`}>
      <Gamepad2 aria-hidden="true" className="text-muted-foreground size-8" />
      <h2 className="font-display text-lg font-semibold">{t("preview.recs.state.noRatingTitle", { keymode })}</h2>
      <p className="text-muted-foreground max-w-md text-sm">{t("preview.recs.state.noRatingBody", { keymode })}</p>
    </div>
  );
}

function Empty({ recs }: { recs: RecsPreviewDto }) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  const [lo, hi] = recs.bandCenti;
  return (
    <div className={`${CARD} items-center py-10 text-center`}>
      <SearchX aria-hidden="true" className="text-muted-foreground size-8" />
      <h2 className="font-display text-lg font-semibold">{t("preview.recs.state.emptyTitle")}</h2>
      <p className="text-muted-foreground max-w-md text-sm">
        {t("preview.recs.state.emptyBody", { skillset: format.skillset(recs.focus), lo: format.approx(lo), hi: format.approx(hi) })}
      </p>
      {recs.anyRate ? (
        <p className="text-muted-foreground max-w-md text-sm">{t("preview.recs.state.emptyTryMode")}</p>
      ) : (
        <>
          <p className="text-muted-foreground max-w-md text-sm">{t("preview.recs.state.emptyAnyRate")}</p>
          <Link to="/settings" className={buttonVariants({ variant: "outline", size: "sm" })}>
            {t("preview.recs.state.enableAnyRate")}
          </Link>
        </>
      )}
    </div>
  );
}

function Picks({
  recs,
  stale,
  onGenerate,
}: {
  recs: RecsPreviewDto;
  stale: boolean;
  onGenerate: ((item: RecItemDto) => void) | undefined;
}) {
  const { t } = useTranslation();
  if (recs.state === "no_rating") {
    return <NoRating keymode={recs.keymode} />;
  }
  if (recs.state === "computing" && recs.items.length === 0) {
    return <Computing />;
  }
  return (
    <>
      <WarningsBanner warnings={recs.warnings} />
      <Summary recs={recs} />
      {recs.state === "computing" && (
        <p role="status" className="text-muted-foreground flex items-center gap-2 text-sm">
          <LoaderCircle aria-hidden="true" className="text-primary size-4 motion-safe:animate-spin" />
          {t("preview.state.updating")}
        </p>
      )}
      {recs.items.length === 0 ? (
        <Empty recs={recs} />
      ) : (
        <ul
          aria-label={t("preview.recs.list")}
          aria-busy={stale}
          className={cn("grid gap-4 lg:grid-cols-2", stale && "opacity-60 motion-safe:transition-opacity")}
        >
          {recs.items.map((item) => (
            <RecCard
              key={`${item.md5}-${String(item.rateMilli)}`}
              item={item}
              focus={recs.focus}
              ratingCenti={recs.ratingCenti}
              band={recs.bandCenti}
              onGenerate={onGenerate}
            />
          ))}
        </ul>
      )}
    </>
  );
}

export interface RecommendationsPageProps {
  /** Hook point for the rate-copies feature: while it is absent, "Generate rate copy" stays disabled. */
  onGenerateRateCopy?: ((item: RecItemDto) => void) | undefined;
}

/** The uncalibrated band recommendations (ADR 0024) for the header's scope, keymode and merge mode. */
export function RecommendationsPage({ onGenerateRateCopy }: RecommendationsPageProps) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const panelId = useId();
  const { active, keymode, mergeMode } = useActiveEntry();
  const [mode, setMode] = useState<RecsModeDto>("deficit");
  const [skillset, setSkillset] = useState<string | null>(null);
  const entry = active?.ref ?? NO_ENTRY;
  const pickable = pickableSkillsets(keymode);
  const enabled = pickable.filter((s) => s.enabled);
  // A keymode switch can leave a pick the new keymode cannot rate.
  const chosen = skillset !== null && enabled.some((s) => s.id === skillset) ? skillset : (enabled[0]?.id ?? null);
  const recs = useQuery({
    ...recsPreviewQuery(entry, keymode, mode, mode === "skillset" ? chosen : null, mergeMode ?? null),
    enabled: active !== undefined,
    // Mode switches keep the previous list on screen; another scope or keymode must not show this one's picks.
    placeholderData: (previous, previousQuery) => (sameView(previousQuery?.queryKey, entry, keymode) ? previous : undefined),
  });
  const calcVersion = recs.data?.calcVersion;

  const changeMode = (next: RecsModeDto) => {
    if (next === "skillset" && mode !== "skillset") {
      const focus = recs.data?.focus;
      setSkillset(enabled.some((s) => s.id === focus) ? (focus ?? null) : (enabled[0]?.id ?? null));
    }
    setMode(next);
  };

  return (
    <>
      <PageHeader
        icon={Compass}
        title={t("preview.recs.title")}
        description={t("preview.recs.subtitle")}
        actions={
          <>
            {calcVersion !== undefined && <BetaBadge calcVersion={calcVersion} />}
            <MethodDisclosure topic="recs" />
          </>
        }
      />
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-6 px-6 py-6">
        <div className="flex flex-col gap-3">
          <ModeTabs mode={mode} panelId={panelId} onChange={changeMode} />
          {mode === "skillset" && <SkillsetPicker keymode={keymode} skillsets={pickable} value={chosen} onChange={setSkillset} />}
        </div>
        <div id={panelId} role="tabpanel" aria-labelledby={`${panelId}-${mode}`} className="flex flex-col gap-6">
          {recs.isError ? (
            <div role="alert" className="bg-destructive/10 text-destructive flex flex-wrap items-center gap-3 rounded-lg px-3 py-2 text-sm">
              <span>{errorText(recs.error)}</span>
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  void recs.refetch();
                }}
              >
                {t("common.retry")}
              </Button>
            </div>
          ) : recs.data === undefined ? (
            <p className="text-muted-foreground text-sm">{t("common.loading")}</p>
          ) : (
            <Picks recs={recs.data} stale={recs.isPlaceholderData} onGenerate={onGenerateRateCopy} />
          )}
        </div>
      </div>
    </>
  );
}
