import { useEffect, useState } from "react";
import type { SkinDto } from "@/ipc/bindings";
import type { LoadedSkin, SkinSlot } from "../skinModel";
import { DEFAULT_SKIN_LOADER_PARAMS, loadSkin, type SkinLoadResult, type SkinLoaderParams } from "./loadSkin";

export interface LoadedSkinState {
  skin: LoadedSkin | null;
  failedSlots: readonly SkinSlot[];
  loading: boolean;
}

const NO_FAILURES: readonly SkinSlot[] = [];

/**
 * Holds the decoded form of one DTO only: GPU-backed bitmaps are closed as soon as the DTO changes or the caller
 * unmounts (ADR 0019), so callers keep the DTO reference stable per (folder, keymode, ini mtime).
 */
export function useLoadedSkin(
  dto: SkinDto | null,
  params: SkinLoaderParams = DEFAULT_SKIN_LOADER_PARAMS,
): LoadedSkinState {
  const [loaded, setLoaded] = useState<{ dto: SkinDto; result: SkinLoadResult } | null>(null);

  useEffect(() => {
    if (dto === null) {
      return;
    }
    let live = true;
    let result: SkinLoadResult | null = null;
    void loadSkin(dto, params).then((r) => {
      if (live) {
        result = r;
        setLoaded({ dto, result: r });
      } else {
        r.dispose();
      }
    });
    return () => {
      live = false;
      if (result !== null) {
        const closed = result;
        closed.dispose();
        // Drop the closed entry so a returning DTO object can never surface its bitmaps again.
        setLoaded((prev) => (prev?.result === closed ? null : prev));
      }
    };
  }, [dto, params]);

  // Until the cleanup's state update lands, the DTO identity check hides an entry whose bitmaps are closed.
  const current = loaded !== null && loaded.dto === dto ? loaded.result : null;
  return {
    skin: current?.skin ?? null,
    failedSlots: current?.failedSlots ?? NO_FAILURES,
    loading: dto !== null && current === null,
  };
}
