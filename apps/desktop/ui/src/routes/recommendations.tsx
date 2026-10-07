import { createFileRoute } from "@tanstack/react-router";
import { Compass } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ComingSoonPage, RecommendationsPreview } from "@/features/roadmap";

export const Route = createFileRoute("/recommendations")({
  component: RecommendationsPage,
});

function RecommendationsPage() {
  const { t } = useTranslation();
  return (
    <ComingSoonPage
      icon={Compass}
      title={t("roadmap.recommendations.title")}
      subtitle={t("roadmap.recommendations.subtitle")}
      preview={<RecommendationsPreview />}
      features={[
        t("roadmap.recommendations.features.picks"),
        t("roadmap.recommendations.features.feedback"),
        t("roadmap.recommendations.features.drills"),
      ]}
    />
  );
}
