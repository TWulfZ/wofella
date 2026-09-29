import { useTranslation } from "react-i18next";
import { useActiveEntry } from "../activeEntry";

// R10: other players' plays never feed the self profile, so any non-self view says so plainly.
export function NotSelfBanner() {
  const { t } = useTranslation();
  const { active } = useActiveEntry();
  if (active === undefined || active.profileKind === "self") {
    return null;
  }
  return (
    <div role="note" className="border-b bg-amber-100 px-6 py-2 text-sm text-amber-950 dark:bg-amber-950 dark:text-amber-100">
      {active.profileKind === "all_players"
        ? t("players.banner.allPlayers")
        : t("players.banner.other", { label: active.label })}
    </div>
  );
}
