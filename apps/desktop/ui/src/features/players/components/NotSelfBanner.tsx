import { Eye, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useActiveEntry } from "../activeEntry";

// R10: other players' plays never feed the self profile, so any non-self view says so plainly.
export function NotSelfBanner() {
  const { t } = useTranslation();
  const { active } = useActiveEntry();
  if (active === undefined || active.profileKind === "self") {
    return null;
  }
  const allPlayers = active.profileKind === "all_players";
  const Icon = allPlayers ? TriangleAlert : Eye;
  return (
    <div role="note" className="bg-warning/10 border-warning/40 text-foreground border-b text-sm">
      <div className="mx-auto flex w-full max-w-5xl items-center gap-2 px-6 py-2">
        <Icon className="text-warning size-4 shrink-0" aria-hidden="true" />
        <span>{allPlayers ? t("players.banner.allPlayers") : t("players.banner.other", { label: active.label })}</span>
      </div>
    </div>
  );
}
