import { queryOptions, skipToken, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type AnchorDto,
  type LabelSubmitDto,
  type SampleRequestDto,
  type SkinEntryDto,
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
  get: (folder: string, keymode: number, iniMtime: string | null) => qk("skins", "get", folder, keymode, iniMtime),
};

export function skinListQuery() {
  return queryOptions({
    queryKey: skinKeys.list(),
    queryFn: () => call(commands.skinList()),
    staleTime: SKIN_QUERY_PARAMS.listStaleTimeMs,
  });
}

export function skinGetQuery(entry: SkinEntryDto | null, keymode: number) {
  return queryOptions({
    // The ini mtime is in the key so an edited skin.ini is a new entry rather than a stale hit (ADR 0019).
    queryKey: skinKeys.get(entry?.folder ?? "", keymode, entry?.iniMtime ?? null),
    queryFn: entry === null ? skipToken : () => call(commands.skinGet(entry.folder, keymode)),
    staleTime: Infinity,
    // Only the skin on screen is kept: its base64 is MBs and its decoded form lives with the screen.
    gcTime: 0,
  });
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
