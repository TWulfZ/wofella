import { createFileRoute } from "@tanstack/react-router";
import { Radar } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ComingSoonPage, SkillPreview } from "@/features/roadmap";

export const Route = createFileRoute("/skill")({
  component: SkillPage,
});

function SkillPage() {
  const { t } = useTranslation();
  return (
    <ComingSoonPage
      icon={Radar}
      title={t("roadmap.skill.title")}
      subtitle={t("roadmap.skill.subtitle")}
      preview={<SkillPreview />}
      features={[t("roadmap.skill.features.axes"), t("roadmap.skill.features.sessions"), t("roadmap.skill.features.self")]}
    />
  );
}
