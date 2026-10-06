import { afterEach, describe, expect, it, vi } from "vitest";
import {
  hasStoredOsuSpeed,
  LABEL_PREFS,
  readSkinChoice,
  readOffsetMs,
  readScrollPrefs,
  readZoom,
  scrollFromPrefs,
  type ScrollPrefs,
  writeFit,
  writeOffsetMs,
  writeOsuSpeed,
  writePxPerMs,
  writeScrollKind,
  writeSkinChoice,
  writeZoom,
} from "./prefs";

const DEFAULTS = { osuSpeed: 20, pxPerMs: 1 };

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("audio offset preference", () => {
  it("defaults to 0 and round-trips a stored value", () => {
    expect(readOffsetMs()).toBe(0);
    writeOffsetMs(-35);
    expect(readOffsetMs()).toBe(-35);
  });

  it("clamps to the slider range and ignores garbage", () => {
    localStorage.setItem(LABEL_PREFS.offsetKey, "999");
    expect(readOffsetMs()).toBe(LABEL_PREFS.maxOffsetMs);
    localStorage.setItem(LABEL_PREFS.offsetKey, "-999");
    expect(readOffsetMs()).toBe(LABEL_PREFS.minOffsetMs);
    localStorage.setItem(LABEL_PREFS.offsetKey, "soon");
    expect(readOffsetMs()).toBe(0);
  });
});

describe("scroll preferences", () => {
  it("default to the osu! mode at the given speed, without fitting", () => {
    expect(readScrollPrefs(DEFAULTS)).toEqual({ kind: "osu", osuSpeed: 20, pxPerMs: 1, fit: false });
    expect(readScrollPrefs({ osuSpeed: 30, pxPerMs: 0.8 })).toEqual({ kind: "osu", osuSpeed: 30, pxPerMs: 0.8, fit: false });
  });

  it("round-trip each field on its own key", () => {
    writeScrollKind("pxPerMs");
    writeOsuSpeed(27);
    writePxPerMs(1.25);
    writeFit(true);
    expect(readScrollPrefs(DEFAULTS)).toEqual({ kind: "pxPerMs", osuSpeed: 27, pxPerMs: 1.25, fit: true });
  });

  it("keep following the default speed until one is chosen", () => {
    writeScrollKind("pxPerMs");
    expect(readScrollPrefs({ osuSpeed: 30, pxPerMs: 1 }).osuSpeed).toBe(30);
  });

  it("migrate the old px/ms preference as the px/ms value, switching to the osu! mode", () => {
    localStorage.setItem(LABEL_PREFS.legacyScrollKey, "1.25");
    expect(readScrollPrefs(DEFAULTS)).toEqual({ kind: "osu", osuSpeed: 20, pxPerMs: 1.25, fit: false });
    localStorage.setItem(LABEL_PREFS.legacyScrollKey, "fit");
    expect(readScrollPrefs(DEFAULTS)).toEqual({ kind: "osu", osuSpeed: 20, pxPerMs: 1, fit: false });
  });

  it("prefer the new px/ms key over the old one", () => {
    localStorage.setItem(LABEL_PREFS.legacyScrollKey, "1.25");
    writePxPerMs(2);
    expect(readScrollPrefs(DEFAULTS).pxPerMs).toBe(2);
  });

  it("clamp stored values and fall back on garbage", () => {
    localStorage.setItem(LABEL_PREFS.osuSpeedKey, "99");
    localStorage.setItem(LABEL_PREFS.pxPerMsKey, "50");
    localStorage.setItem(LABEL_PREFS.scrollKindKey, "lazer");
    expect(readScrollPrefs(DEFAULTS)).toMatchObject({ kind: "osu", osuSpeed: 40, pxPerMs: LABEL_PREFS.maxPxPerMs });
    localStorage.setItem(LABEL_PREFS.osuSpeedKey, "0");
    localStorage.setItem(LABEL_PREFS.pxPerMsKey, "-1");
    expect(readScrollPrefs(DEFAULTS)).toMatchObject({ osuSpeed: 1, pxPerMs: LABEL_PREFS.minPxPerMs });
    localStorage.setItem(LABEL_PREFS.osuSpeedKey, "fast");
    localStorage.setItem(LABEL_PREFS.pxPerMsKey, "fast");
    localStorage.setItem(LABEL_PREFS.fitKey, "maybe");
    expect(readScrollPrefs(DEFAULTS)).toEqual({ kind: "osu", osuSpeed: 20, pxPerMs: 1, fit: false });
  });

  it("turn into the playfield's scroll mode", () => {
    const prefs: ScrollPrefs = { kind: "osu", osuSpeed: 27, pxPerMs: 1.25, fit: false };
    expect(scrollFromPrefs(prefs)).toEqual({ kind: "osu", speed: 27 });
    expect(scrollFromPrefs({ ...prefs, kind: "pxPerMs" })).toEqual({ kind: "pxPerMs", value: 1.25 });
    expect(scrollFromPrefs({ ...prefs, fit: true })).toBe("fit");
  });
});

describe("zoom preference", () => {
  it("defaults to 1, round-trips and clamps to 0.75..2", () => {
    expect(readZoom()).toBe(1);
    writeZoom(1.5);
    expect(readZoom()).toBe(1.5);
    localStorage.setItem(LABEL_PREFS.zoomKey, "9");
    expect(readZoom()).toBe(2);
    localStorage.setItem(LABEL_PREFS.zoomKey, "0.1");
    expect(readZoom()).toBe(0.75);
    localStorage.setItem(LABEL_PREFS.zoomKey, "big");
    expect(readZoom()).toBe(1);
  });
});

describe("unavailable storage", () => {
  it("reads defaults and drops writes instead of throwing", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    });
    expect(readOffsetMs()).toBe(0);
    expect(readScrollPrefs(DEFAULTS)).toEqual({ kind: "osu", osuSpeed: 20, pxPerMs: 1, fit: false });
    expect(readZoom()).toBe(1);
    expect(() => {
      writeOffsetMs(10);
      writeScrollKind("pxPerMs");
      writeOsuSpeed(30);
      writePxPerMs(1);
      writeFit(true);
      writeZoom(1.5);
    }).not.toThrow();
  });
});

describe("osu! speed choice", () => {
  it("counts as chosen only once a usable speed is stored", () => {
    expect(hasStoredOsuSpeed()).toBe(false);
    localStorage.setItem(LABEL_PREFS.osuSpeedKey, "soon");
    expect(hasStoredOsuSpeed()).toBe(false);
    writeOsuSpeed(27);
    expect(hasStoredOsuSpeed()).toBe(true);
  });
});

describe("skin choice", () => {
  it("is unset until written, and round-trips a folder or None", () => {
    expect(readSkinChoice()).toBeUndefined();
    writeSkinChoice({ folder: "none" });
    expect(readSkinChoice()).toEqual({ folder: "none" });
    writeSkinChoice({ folder: null });
    expect(readSkinChoice()).toEqual({ folder: null });
  });

  it("reads garbage as unset", () => {
    for (const raw of ["Pilot Skin", "{}", '{"folder":3}', "null"]) {
      localStorage.setItem(LABEL_PREFS.skinKey, raw);
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
    expect(hasStoredOsuSpeed()).toBe(false);
    expect(() => {
      writeSkinChoice({ folder: "A" });
    }).not.toThrow();
  });
});
