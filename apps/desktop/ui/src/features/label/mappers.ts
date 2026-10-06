import type { ChartWindowDto, LabelWindowDto } from "@/ipc/bindings";
import type { ChartWindow } from "@/features/playfield";
import type { LabelWindow } from "./types";

export function toChartWindow(dto: ChartWindowDto): ChartWindow {
  return {
    md5: dto.md5,
    keymode: dto.keymode,
    fromMs: dto.fromMs,
    toMs: dto.toMs,
    notes: dto.notes.map(({ tMs, col, endMs }) => ({ tMs, col, endMs })),
    timing: dto.timing.map(({ tMs, kind, beatLenMs, meter, sv }) => ({ tMs, kind, beatLenMs, meter, sv })),
    layout: { id: dto.layout.id, columns: dto.layout.columns.map(({ hand, finger }) => ({ hand, finger })) },
    chartSpan: { firstMs: dto.chartSpan.firstMs, endMs: dto.chartSpan.endMs },
    audioFilename: dto.audioFilename,
  };
}

export function toLabelWindow(dto: LabelWindowDto): LabelWindow {
  return {
    anchor: { md5: dto.anchor.md5, t0Ms: dto.anchor.t0Ms, t1Ms: dto.anchor.t1Ms, cols: [...dto.anchor.cols] },
    title: dto.title,
    artist: dto.artist,
    version: dto.version,
    level: dto.level,
    stratum: dto.stratum,
    played: dto.played,
  };
}
