import { createFileRoute } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

// Placeholder; 005 T15 replaces it with the home status screen.
export const Route = createFileRoute("/")({
  component: HomePage,
});

function HomePage() {
  const { t } = useTranslation();
  return (
    <main className="p-6">
      <h1 className="text-2xl font-semibold">{t("common.home.title")}</h1>
      <p className="text-muted-foreground">{t("common.home.placeholder")}</p>
    </main>
  );
}
