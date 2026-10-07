import { queryOptions, skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import {
  commands,
  type LabelSubmitDto,
  type MoveWindowRequestDto,
  type NowPlayingRequestDto,
  type RandomRequestDto,
  type ResizeWindowRequestDto,
  type SampleRequestDto,
  type SkinDto,
  type SkinListDto,
} from "@/ipc/bindings";
import { call } from "@/ipc/client";
import type { ChartWindow } from "@/features/playfield";
import { qk } from "@/shared/queryKeys";
import { toChartWindow, toLabelWindow } from "./mappers";
import type { Anchor } from "./types";

export const labelKeys = {
  all: qk("labels"),
  taxonomy: (keymode: number) => qk("labels", "taxonomy", keymode),
  /** Keyed by the hand layout they are drawn with, so a changed preference fetches them again. */
  patternExamples: (keymode: number, layoutId: string | null) => qk("labels", "patternExamples", keymode, layoutId),
  stats: () => qk("labels", "stats"),
  chartWindow: (md5: string, fromMs: number, toMs: number, layoutId: string | null) =>
    qk("labels", "chartWindow", md5, fromMs, toMs, layoutId),
  chartAudio: (md5: string) => qk("labels", "chartAudio", md5),
  chartBackground: (md5: string) => qk("labels", "chartBackground", md5),
  chartDetails: (md5: string) => qk("labels", "chartDetails", md5),
  chartTimelines: () => qk("labels", "chartTimeline"),
  chartTimeline: (keymode: number, md5: string, buckets: number) => qk("labels", "chartTimeline", keymode, md5, buckets),
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

/** A null `layoutId` draws with the stored preference, which is also what the key stands for. */
export function labelPatternExamplesQuery(keymode: number, layoutId: string | null) {
  return queryOptions({
    queryKey: labelKeys.patternExamples(keymode, layoutId),
    queryFn: async (): Promise<ReadonlyMap<string, ChartWindow>> => {
      const examples = await call(commands.labelPatternExamples(keymode, layoutId));
      return new Map(examples.map(({ id, window }) => [id, toChartWindow(window)]));
    },
    staleTime: Infinity,
  });
}

export function labelStatsQuery() {
  return queryOptions({ queryKey: labelKeys.stats(), queryFn: () => call(commands.labelStats()) });
}

/** A null `layoutId` draws with the profile's default layout. */
export function chartWindowQuery(anchor: Anchor | null, layoutId: string | null) {
  return queryOptions({
    queryKey: labelKeys.chartWindow(anchor?.md5 ?? "", anchor?.t0Ms ?? 0, anchor?.t1Ms ?? 0, layoutId),
    queryFn:
      anchor === null
        ? skipToken
        : async () => toChartWindow(await call(commands.chartWindow(anchor.md5, anchor.t0Ms, anchor.t1Ms, layoutId))),
    // A move keeps the previous notes on screen until the new span arrives; a new chart must not show the old one.
    placeholderData: (previous) => (previous?.md5 === anchor?.md5 ? previous : undefined),
  });
}

export function chartAudioQuery(md5: string | null) {
  return queryOptions({
    queryKey: labelKeys.chartAudio(md5 ?? ""),
    queryFn: md5 === null ? skipToken : () => call(commands.chartAudio(md5)),
    // Keyed by md5 only, so window moves reuse the bytes; dropped as soon as the round moves on, since a song is MBs.
    staleTime: Infinity,
    gcTime: 0,
  });
}

export function chartBackgroundQuery(md5: string | null) {
  return queryOptions({
    queryKey: labelKeys.chartBackground(md5 ?? ""),
    queryFn: md5 === null ? skipToken : () => call(commands.chartBackground(md5)),
    staleTime: Infinity,
    // An image is MBs of base64; Previous refetches it rather than keeping every chart's.
    gcTime: 0,
  });
}

export function chartDetailsQuery(md5: string | null) {
  return queryOptions({
    queryKey: labelKeys.chartDetails(md5 ?? ""),
    queryFn: md5 === null ? skipToken : () => call(commands.chartDetails(md5)),
    staleTime: Infinity,
  });
}

export function chartTimelineQuery(keymode: number, md5: string | null, buckets: number) {
  return queryOptions({
    queryKey: labelKeys.chartTimeline(keymode, md5 ?? "", buckets),
    queryFn: md5 === null ? skipToken : () => call(commands.labelChartTimeline({ keymode, md5, buckets })),
    // Labelled spans change only through this screen's saves and undos, which invalidate it.
    staleTime: Infinity,
  });
}

export function useLabelMutations() {
  const queryClient = useQueryClient();
  const refreshLabelled = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: labelKeys.stats() }),
      queryClient.invalidateQueries({ queryKey: labelKeys.chartTimelines() }),
    ]);
  return {
    sample: useMutation({
      mutationFn: async (req: SampleRequestDto) => {
        const window = await call(commands.labelSample(req));
        return window === null ? null : toLabelWindow(window);
      },
    }),
    random: useMutation({
      mutationFn: async (req: RandomRequestDto) => {
        const window = await call(commands.labelRandom(req));
        return window === null ? null : toLabelWindow(window);
      },
    }),
    nowPlaying: useMutation({
      mutationFn: async (req: NowPlayingRequestDto) => {
        const found = await call(commands.labelNowPlaying(req));
        return found === null ? null : { window: toLabelWindow(found.window), source: found.source };
      },
    }),
    move: useMutation({ mutationFn: (req: MoveWindowRequestDto) => call(commands.labelMoveWindow(req)) }),
    resize: useMutation({ mutationFn: (req: ResizeWindowRequestDto) => call(commands.labelResizeWindow(req)) }),
    submit: useMutation({ mutationFn: (req: LabelSubmitDto) => call(commands.labelSubmit(req)), onSuccess: refreshLabelled }),
    undo: useMutation({ mutationFn: (eventId: string) => call(commands.labelUndo(eventId)), onSuccess: refreshLabelled }),
  };
}
