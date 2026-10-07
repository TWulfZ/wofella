import { describe, expect, it } from "vitest";
import type { ChartWindowDto, LabelWindowDto } from "@/ipc/bindings";
import { toChartWindow, toLabelWindow } from "./mappers";

const CHART: ChartWindowDto = {
  md5: "0123456789abcdef0123456789abcdef",
  keymode: 7,
  fromMs: 1000,
  toMs: 5000,
  notes: [
    { tMs: 900, col: 3, endMs: 1500 },
    { tMs: 1000, col: 0, endMs: null },
  ],
  timing: [
    { tMs: 0, kind: "red", beatLenMs: 300, meter: 4, sv: null },
    { tMs: 2000, kind: "green", beatLenMs: null, meter: null, sv: 0.8 },
  ],
  layout: {
    id: "k7.313_right_thumb",
    columns: [
      { hand: "left", finger: "ring" },
      { hand: "both", finger: "thumb" },
    ],
  },
  chartSpan: { firstMs: 500, endMs: 90_000 },
  audioFilename: "audio.mp3",
};

describe("toChartWindow", () => {
  it("carries every field the playfield draws", () => {
    expect(toChartWindow(CHART)).toEqual({
      md5: CHART.md5,
      keymode: 7,
      fromMs: 1000,
      toMs: 5000,
      notes: [
        { tMs: 900, col: 3, endMs: 1500 },
        { tMs: 1000, col: 0, endMs: null },
      ],
      timing: [
        { tMs: 0, kind: "red", beatLenMs: 300, meter: 4, sv: null },
        { tMs: 2000, kind: "green", beatLenMs: null, meter: null, sv: 0.8 },
      ],
      layout: {
        id: "k7.313_right_thumb",
        columns: [
          { hand: "left", finger: "ring" },
          { hand: "both", finger: "thumb" },
        ],
      },
      chartSpan: { firstMs: 500, endMs: 90_000 },
      audioFilename: "audio.mp3",
    });
  });

  it("does not share arrays with the DTO", () => {
    const out = toChartWindow(CHART);
    expect(out.notes).not.toBe(CHART.notes);
    expect(out.layout.columns).not.toBe(CHART.layout.columns);
  });
});

describe("toLabelWindow", () => {
  it("keeps the anchor and the chart header", () => {
    const dto: LabelWindowDto = {
      anchor: { md5: CHART.md5, t0Ms: 1000, t1Ms: 5000, cols: [1, 4] },
      title: "Title",
      artist: "Artist",
      version: "Insane",
      creator: "Mapper",
      stars: 4.52,
      level: "dan:7",
      stratum: "dan_07/nps_2",
      played: true,
    };
    expect(toLabelWindow(dto)).toEqual(dto);
  });
});
