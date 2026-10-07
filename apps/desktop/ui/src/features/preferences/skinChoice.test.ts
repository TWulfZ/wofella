import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChartWindow } from "@/features/playfield";
import type { SkinEntryDto, SkinListDto } from "@/ipc/bindings";
import {
  previewExample,
  readSkinChoice,
  selectedSkinFolder,
  SKIN_CHOICE_KEY,
  skinOptions,
  writeSkinChoice,
} from "./skinChoice";

function entry(folder: string, keymodes: number[]): SkinEntryDto {
  return { folder, name: folder, keymodes, iniMtime: "1" };
}

const LIST: SkinListDto = {
  skins: [entry("Alpha4K", [4]), entry("Pilot", [4, 7]), entry("Zeta", [7])],
  current: "Pilot",
  maniaSpeed: null,
  maniaSpeedBpmScale: null,
};

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("skin choice storage", () => {
  it("lives under the key the Label screen reads, as JSON", () => {
    expect(SKIN_CHOICE_KEY).toBe("wolluf.label.skin");
    writeSkinChoice({ folder: "Zeta" });
    expect(localStorage.getItem("wolluf.label.skin")).toBe('{"folder":"Zeta"}');
    writeSkinChoice({ folder: null });
    expect(localStorage.getItem("wolluf.label.skin")).toBe('{"folder":null}');
  });

  it("is unset until written, and round-trips a folder or None", () => {
    expect(readSkinChoice()).toBeUndefined();
    writeSkinChoice({ folder: "none" });
    expect(readSkinChoice()).toEqual({ folder: "none" });
    writeSkinChoice({ folder: null });
    expect(readSkinChoice()).toEqual({ folder: null });
  });

  it("reads garbage as unset", () => {
    for (const raw of ["Pilot Skin", "{}", '{"folder":3}', "null"]) {
      localStorage.setItem(SKIN_CHOICE_KEY, raw);
      expect(readSkinChoice(), raw).toBeUndefined();
    }
  });

  it("survives a storage that throws", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    });
    expect(readSkinChoice()).toBeUndefined();
    expect(() => {
      writeSkinChoice({ folder: "A" });
    }).not.toThrow();
  });
});

describe("skinOptions", () => {
  it("puts the skins with a block for the keymode first", () => {
    expect(skinOptions(LIST, 7).map((o) => [o.entry.folder, o.hasKeymode])).toEqual([
      ["Pilot", true],
      ["Zeta", true],
      ["Alpha4K", false],
    ]);
    expect(skinOptions(undefined, 7)).toEqual([]);
  });
});

describe("selectedSkinFolder", () => {
  it("follows the cfg skin until a choice is stored", () => {
    expect(selectedSkinFolder(LIST, undefined)).toBe("Pilot");
    expect(selectedSkinFolder({ ...LIST, current: null }, undefined)).toBeNull();
  });

  it("keeps a stored folder or None, and drops a folder that is gone", () => {
    expect(selectedSkinFolder(LIST, { folder: "Zeta" })).toBe("Zeta");
    expect(selectedSkinFolder(LIST, { folder: null })).toBeNull();
    expect(selectedSkinFolder(LIST, { folder: "Gone" })).toBe("Pilot");
  });

  it("trusts a stored choice before the list answers", () => {
    expect(selectedSkinFolder(undefined, { folder: "Zeta" })).toBe("Zeta");
    expect(selectedSkinFolder(undefined, undefined)).toBeNull();
  });
});

describe("previewExample", () => {
  const window = (fromMs: number): ChartWindow => ({
    md5: "0".repeat(32),
    keymode: 7,
    fromMs,
    toMs: fromMs + 1000,
    notes: [],
    timing: [],
    layout: { id: "k7.313_right_thumb", columns: [] },
    chartSpan: { firstMs: 0, endMs: fromMs + 1000 },
    audioFilename: null,
  });

  it("prefers the jumpstream example", () => {
    const jumpstream = window(2);
    const examples = new Map([
      ["regular.jack.minijack", window(1)],
      ["regular.stream.jumpstream", jumpstream],
    ]);
    expect(previewExample(examples)).toBe(jumpstream);
  });

  it("falls back to any example, and to none while they load", () => {
    const minijack = window(1);
    expect(previewExample(new Map([["regular.jack.minijack", minijack]]))).toBe(minijack);
    expect(previewExample(new Map())).toBeUndefined();
    expect(previewExample(undefined)).toBeUndefined();
  });
});
