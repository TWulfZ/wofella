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

interface SharedLoad {
  promise: Promise<SkinLoadResult>;
  result: SkinLoadResult | null;
  users: number;
}

// StrictMode releases and re-acquires before the decode finishes, so a load outlives a zero count until it settles.
const loads = new WeakMap<SkinLoaderParams, WeakMap<SkinDto, SharedLoad>>();

function acquire(dto: SkinDto, params: SkinLoaderParams): SharedLoad {
  let byDto = loads.get(params);
  if (byDto === undefined) {
    byDto = new WeakMap();
    loads.set(params, byDto);
  }
  let load = byDto.get(dto);
  if (load === undefined) {
    const created: SharedLoad = { promise: loadSkin(dto, params), result: null, users: 0 };
    const owner = byDto;
    void created.promise.then((r) => {
      created.result = r;
      if (created.users === 0) {
        r.dispose();
        owner.delete(dto);
      }
    });
    byDto.set(dto, created);
    load = created;
  }
  load.users++;
  return load;
}

function release(load: SharedLoad, dto: SkinDto, params: SkinLoaderParams): void {
  load.users--;
  if (load.users === 0 && load.result !== null) {
    load.result.dispose();
    // A returning DTO object then decodes afresh instead of surfacing closed bitmaps.
    loads.get(params)?.delete(dto);
  }
}

/**
 * Holds the decoded form of one DTO only: GPU-backed bitmaps are closed as soon as the DTO changes or the last caller
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
    const load = acquire(dto, params);
    void load.promise.then((r) => {
      if (live) {
        setLoaded({ dto, result: r });
      }
    });
    return () => {
      live = false;
      const closed = load.result;
      release(load, dto, params);
      if (closed !== null) {
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
