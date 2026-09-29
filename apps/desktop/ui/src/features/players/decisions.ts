import type { AliasDecisionInput, AliasRowDto } from "@/ipc/bindings";

/**
 * Spec 004 Behaviour 4: ticked → me; a row the backend had selected (auto or `me`) that is now unticked → not_me;
 * every other unticked row gets no decision, so the auto rule keeps owning it.
 */
export function buildDecisions(rows: readonly AliasRowDto[], ticked: ReadonlySet<number>): AliasDecisionInput[] {
  return rows.flatMap((row): AliasDecisionInput[] => {
    if (ticked.has(row.aliasId)) {
      return [{ aliasId: row.aliasId, decision: "me" }];
    }
    return row.selected ? [{ aliasId: row.aliasId, decision: "not_me" }] : [];
  });
}
