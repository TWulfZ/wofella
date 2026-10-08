import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { RecommendationsPage } from "@/features/preview";
import { RateCopyDialog, type RateCopyTarget } from "@/features/rateCopies";

function RecommendationsRoute() {
  const [target, setTarget] = useState<RateCopyTarget | null>(null);
  return (
    <>
      <RecommendationsPage
        onGenerateRateCopy={(item) => {
          setTarget({ md5: item.md5, rateMilli: item.rateMilli, chartLabel: `${item.artist} - ${item.title} [${item.version}]` });
        }}
      />
      <RateCopyDialog
        target={target}
        onOpenChange={(open) => {
          if (!open) {
            setTarget(null);
          }
        }}
      />
    </>
  );
}

export const Route = createFileRoute("/recommendations")({
  component: RecommendationsRoute,
});
