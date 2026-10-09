import { Gamepad2, LoaderCircle } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { SkillPreviewDto } from "@/ipc/bindings";
import { usePreviewFormat } from "../format";
import { radarScale, unmeasuredSkillsets } from "../model";
import { DanChip } from "./DanChip";
import { EvidencePanel } from "./EvidencePanel";
import { Heading, type HeadingLevel } from "./Heading";
import { SkillRadar } from "./SkillRadar";
import { SkillsetList } from "./SkillsetList";
import { TopPlaysTable } from "./TopPlaysTable";
import { TrendChart } from "./TrendChart";
import { WarningsBanner } from "./WarningsBanner";

const CARD = "bg-card ring-border flex flex-col gap-4 rounded-xl p-5 shadow-lg shadow-black/20 ring-1";

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

function NoPlays({ preview, level }: { preview: SkillPreviewDto; level: HeadingLevel }) {
  const { t } = useTranslation();
  return (
    <>
      <div className={`${CARD} items-center py-10 text-center`}>
        <Gamepad2 aria-hidden="true" className="text-muted-foreground size-8" />
        <Heading level={level} className="text-lg">
          {t("preview.state.noPlaysTitle", { keymode: preview.keymode })}
        </Heading>
        <p className="text-muted-foreground max-w-md text-sm">{t("preview.state.noPlaysBody", { keymode: preview.keymode })}</p>
      </div>
      {preview.evidence.excluded.length > 0 && <EvidencePanel evidence={preview.evidence} level={level} />}
    </>
  );
}

function Updating() {
  const { t } = useTranslation();
  return (
    <p role="status" className="text-muted-foreground flex items-center gap-2 text-sm">
      <LoaderCircle aria-hidden="true" className="text-primary size-4 motion-safe:animate-spin" />
      {t("preview.state.updating")}
    </p>
  );
}

function Ready({ preview, level }: { preview: SkillPreviewDto; level: HeadingLevel }) {
  const { t } = useTranslation();
  const format = usePreviewFormat();
  const trendId = useId();
  const skillsetsId = useId();
  // Overall arrives separately as `overallCenti`; a stray `overall` row must not become an axis.
  const skillsets = preview.skillsets.filter((s) => s.id !== "overall");
  const unmeasured = unmeasuredSkillsets(preview.warnings);
  const scale = radarScale(skillsets.filter((s) => !unmeasured.has(s.id)).map((s) => s.ratingCenti));
  const pending = preview.evidence.excluded.find((e) => e.reason === "pending")?.count ?? 0;
  const preparing = preview.evidence.counted === 0 && pending > 0;
  return (
    <>
      <WarningsBanner warnings={preview.warnings} />
      <div className={`${CARD} osu-triangles grid items-center gap-6 md:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]`}>
        <div className="flex flex-col gap-3">
          {preview.state === "computing" && <Updating />}
          <span className="text-muted-foreground text-sm">{t("preview.overall")}</span>
          {preview.overallCenti !== null && (
            <p data-testid="overall" className="font-display text-6xl leading-none font-extrabold tracking-tight italic tabular-nums">
              {format.approx(preview.overallCenti)}
            </p>
          )}
          {preview.dan !== null && <DanChip dan={preview.dan} />}
          <p className="text-muted-foreground text-sm">
            {t("preview.evidence.counted", { count: preview.evidence.counted })} ·{" "}
            {t(`preview.evidence.tierName.${preview.evidence.tier}`)}
          </p>
        </div>
        <div className="flex justify-center">
          {preparing ? (
            <p className="text-muted-foreground max-w-xs text-center text-sm">{t("preview.state.preparing", { count: pending })}</p>
          ) : (
            <SkillRadar skillsets={skillsets} unmeasured={unmeasured} scale={scale} />
          )}
        </div>
      </div>
      <div className="grid gap-6 lg:grid-cols-2">
        <section aria-labelledby={skillsetsId} className={CARD}>
          <Heading level={level} id={skillsetsId}>
            {t("preview.skillsets.title")}
          </Heading>
          <SkillsetList skillsets={skillsets} unmeasured={unmeasured} maxCenti={scale.hi} />
        </section>
        <div className="flex min-w-0 flex-col gap-6">
          <section aria-labelledby={trendId} className={CARD}>
            <Heading level={level} id={trendId}>
              {t("preview.trend.title")}
            </Heading>
            <TrendChart trend={preview.trend} />
          </section>
          <EvidencePanel evidence={preview.evidence} level={level} />
        </div>
      </div>
      <TopPlaysTable plays={preview.topPlays} level={level} />
    </>
  );
}

function hasCachedRatings(preview: SkillPreviewDto): boolean {
  return preview.overallCenti !== null || preview.topPlays.length > 0;
}

export function ScopePreview({ preview, name }: { preview: SkillPreviewDto; name?: string | undefined }) {
  const headingId = useId();
  const level: HeadingLevel = name === undefined ? 2 : 3;
  const body =
    preview.state === "computing" && !hasCachedRatings(preview) ? (
      <Computing />
    ) : preview.state === "no_plays" ? (
      <NoPlays preview={preview} level={level} />
    ) : (
      <Ready preview={preview} level={level} />
    );
  if (name === undefined) {
    return <div className="flex flex-col gap-6">{body}</div>;
  }
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-6">
      <h2 id={headingId} className="font-display border-osu-pink border-l-4 pl-3 text-xl font-bold">
        {name}
      </h2>
      {body}
    </section>
  );
}
