import { queryOptions, skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import {
  commands,
  type AnchorDto,
  type LabelSubmitDto,
  type SampleRequestDto,
  type SkinDto,
  type SkinListDto,
  type WindowOpDto,
} from "@/ipc/bindings";
import { call } from "@/ipc/client";
import { qk } from "@/shared/queryKeys";
import { toChartWindow, toLabelWindow } from "./mappers";
import type { Anchor } from "./types";

export const labelKeys = {
  all: qk("labels"),
  taxonomy: (keymode: number) => qk("labels", "taxonomy", keymode),
  stats: () => qk("labels", "stats"),
  chartWindow: (md5: string, fromMs: number, toMs: number) => qk("labels", "chartWindow", md5, fromMs, toMs),
  chartAudio: (md5: string) => qk("labels", "chartAudio", md5),
};

export const SKIN_QUERY_PARAMS = {
  /** A skin added or switched in osu! shows up without a restart; Reload skin is the immediate path. */
  listStaleTimeMs: 60_000,
} as const;

export const skinKeys = {
  all: qk("skins"),
  list: () => qk("skins", "list"),
  get: (folder: string, keymode: number) => qk("skins", "get", folder, keymode),
};

export function skinListQuery() {
  return queryOptions({
    queryKey: skinKeys.list(),
    queryFn: () => call(commands.skinList()),
    staleTime: SKIN_QUERY_PARAMS.listStaleTimeMs,
  });
}

export interface SkinFile {
  dto: SkinDto;
  /**
   * The listed skin.ini mtime when the fetch started; undefined when the list had not answered yet, since listing every
   * skin folder is slow and the first paint does not wait for it.
   */
  iniMtime: string | null | undefined;
}

/** The listed skin.ini mtime of a folder; undefined while the list is unknown or does not name the folder. */
export function listedIniMtime(list: SkinListDto | undefined, folder: string): string | null | undefined {
  return list?.skins.find((s) => s.folder === folder)?.iniMtime;
}

export function skinGetQuery(folder: string | null, keymode: number) {
  return queryOptions({
    // Keyed without the ini mtime so the first paint need not wait for the list; useSkinFile refetches on a change.
    queryKey: skinKeys.get(folder ?? "", keymode),
    queryFn:
      folder === null
        ? skipToken
        : async ({ client }): Promise<SkinFile> => {
            const iniMtime = listedIniMtime(client.getQueryData(skinKeys.list()), folder);
            return { dto: await call(commands.skinGet(folder, keymode)), iniMtime };
          },
    staleTime: Infinity,
    // Only the skin on screen is kept: its base64 is MBs and its decoded form lives with the screen.
    gcTime: 0,
  });
}

/**
 * One skin_get per (folder, keymode, ini mtime) as ADR 0019 asks, without putting the mtime in the key: a fetch that
 * beat the list adopts the list's mtime, and a later different mtime refetches once.
 */
export function useSkinFile(folder: string | null, keymode: number, list: SkinListDto | undefined) {
  const queryClient = useQueryClient();
  const query = useQuery(skinGetQuery(folder, keymode));
  const listed = folder === null ? undefined : listedIniMtime(list, folder);
  const fetchedWith = query.data?.iniMtime;
  const hasData = query.data !== undefined;
  const { refetch } = query;
  useEffect(() => {
    if (folder === null || !hasData || listed === undefined || fetchedWith === listed) {
      return;
    }
    if (fetchedWith === undefined) {
      // A skin.ini edited between the two concurrent reads is missed until Reload; the window is one IPC round trip.
      queryClient.setQueryData<SkinFile>(skinKeys.get(folder, keymode), (prev) =>
        prev === undefined ? prev : { ...prev, iniMtime: listed },
      );
      return;
    }
    // Joins a fetch already in flight (Reload's) instead of cancelling it and asking the disk twice.
    void refetch({ cancelRefetch: false });
  }, [queryClient, folder, keymode, listed, fetchedWith, hasData, refetch]);
  return query;
}

export function labelTaxonomyQuery(keymode: number) {
  return queryOptions({
    queryKey: labelKeys.taxonomy(keymode),
    queryFn: () => call(commands.labelTaxonomy(keymode)),
    staleTime: Infinity,
  });
}

export function labelStatsQuery() {
  return queryOptions({ queryKey: labelKeys.stats(), queryFn: () => call(commands.labelStats()) });
}

export function chartWindowQuery(anchor: Anchor | null) {
  return queryOptions({
    queryKey: labelKeys.chartWindow(anchor?.md5 ?? "", anchor?.t0Ms ?? 0, anchor?.t1Ms ?? 0),
    queryFn:
      anchor === null
        ? skipToken
        : async () => toChartWindow(await call(commands.chartWindow(anchor.md5, anchor.t0Ms, anchor.t1Ms, null))),
    // A reshape keeps the previous notes on screen until the new span arrives; a new chart must not show the old one.
    placeholderData: (previous) => (previous?.md5 === anchor?.md5 ? previous : undefined),
  });
}

export function chartAudioQuery(md5: string | null) {
  return queryOptions({
    queryKey: labelKeys.chartAudio(md5 ?? ""),
    queryFn: md5 === null ? skipToken : () => call(commands.chartAudio(md5)),
    // Keyed by md5 only, so reshapes reuse the bytes; dropped as soon as the round moves on, since a song is MBs.
    staleTime: Infinity,
    gcTime: 0,
  });
}

export function useLabelMutations(keymode: number) {
  const queryClient = useQueryClient();
  const refreshStats = () => queryClient.invalidateQueries({ queryKey: labelKeys.stats() });
  return {
    sample: useMutation({
      mutationFn: async (req: SampleRequestDto) => {
        const window = await call(commands.labelSample(req));
        return window === null ? null : toLabelWindow(window);
      },
    }),
    resolve: useMutation({ mutationFn: (tokens: string[]) => call(commands.labelResolvePatterns(keymode, tokens)) }),
    reshape: useMutation({
      mutationFn: ({ anchor, op }: { anchor: AnchorDto; op: WindowOpDto }) => call(commands.labelReshape(anchor, op)),
    }),
    submit: useMutation({ mutationFn: (req: LabelSubmitDto) => call(commands.labelSubmit(req)), onSuccess: refreshStats }),
    undo: useMutation({ mutationFn: (eventId: string) => call(commands.labelUndo(eventId)), onSuccess: refreshStats }),
  };
}

/** Writes the gold set into the data dir's exports folder and opens it; a "Save as" dialog would need a new capability. */
export function useLabelExport(keymode: number) {
  return useMutation({
    mutationFn: async () => {
      const exported = await call(commands.labelExport(keymode));
      await call(commands.appOpenExportsDir());
      return exported;
    },
  });
}
