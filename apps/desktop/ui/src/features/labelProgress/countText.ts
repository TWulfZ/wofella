import type { TFunction } from "i18next";
import type { SessionMapCounts } from "./model";

/** Each number is pluralized on its own, so "1 pending" and "2 pendientes" both read right. */
export function sessionCountsText(
  t: TFunction,
  key: "labelProgress.session.counts" | "labelProgress.summary.sessionValue",
  { labelled, pending }: SessionMapCounts,
): string {
  return t(key, {
    labelled: t("labelProgress.count.labelled", { count: labelled }),
    pending: t("labelProgress.count.pending", { count: pending }),
  });
}
