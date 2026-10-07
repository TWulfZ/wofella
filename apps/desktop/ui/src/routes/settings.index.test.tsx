import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bootApp } from "@/app/testing";
import { skinKeys } from "@/features/label";
import type { ChartWindowDto, HandLayoutDto, PatternExampleDto, SkinDto, SkinEntryDto, SkinListDto } from "@/ipc/bindings";
import { mockIpcError, type CommandHandlers } from "@/ipc/mocks";
import { i18n, LANGUAGE_STORAGE_KEY } from "@/shared/i18n";

// Hands only: these tests cover naming and saving; the diagram's fingers are covered by LayoutDiagram's own tests.
function preset(id: string, hands: string): HandLayoutDto {
  return {
    id,
    columns: hands.split(" ").map((h) => ({ hand: h === "L" ? "left" : h === "R" ? "right" : "both", finger: "index" })),
  };
}

const K7_LAYOUTS = [
  preset("k7.313_right_thumb", "L L L R R R R"),
  preset("k7.313_left_thumb", "L L L L R R R"),
  preset("k7.43", "L L L L R R R"),
  preset("k7.34", "L L L R R R R"),
  preset("k7.both_thumbs", "L L L B R R R"),
];

const NO_SKINS: SkinListDto = { skins: [], current: null, maniaSpeed: null, maniaSpeedBpmScale: null };

function exampleWindow(fromMs: number, toMs: number): ChartWindowDto {
  return {
    md5: "0".repeat(32),
    keymode: 7,
    fromMs,
    toMs,
    notes: [
      { tMs: fromMs, col: 3, endMs: null },
      { tMs: fromMs + 150, col: 1, endMs: null },
    ],
    timing: [{ tMs: 0, kind: "red", beatLenMs: 300, meter: 4, sv: null }],
    layout: { id: "k7.313_right_thumb", columns: K7_LAYOUTS[0]?.columns ?? [] },
    chartSpan: { firstMs: 0, endMs: toMs },
    audioFilename: null,
  };
}

const EXAMPLES: PatternExampleDto[] = [
  { id: "regular.jack.minijack", window: exampleWindow(0, 1500) },
  { id: "regular.stream.jumpstream", window: exampleWindow(0, 1900) },
];

function handLayoutHandlers(initial = "k7.313_right_thumb"): CommandHandlers {
  let current = initial;
  return {
    // The default-skin card shares the page; with no skins it stays quiet and procedural.
    skinList: () => NO_SKINS,
    labelPatternExamples: () => EXAMPLES,
    settingsHandLayouts: () => K7_LAYOUTS,
    settingsGetHandLayout: () => current,
    settingsSetHandLayout: (args) => {
      current = String(args["layoutId"]);
      return null;
    },
  };
}

// The page always renders the default-skin preview, so every test needs a sized, drawable canvas.
interface RecordingCanvas {
  images: unknown[];
  fills: number;
}
let canvas: RecordingCanvas;
let bitmaps: { close: ReturnType<typeof vi.fn> }[];

class SizedResizeObserver {
  constructor(private readonly callback: ResizeObserverCallback) {}
  observe(target: Element): void {
    const entry = { target, contentRect: { width: 400, height: 300 } };
    queueMicrotask(() => {
      this.callback([entry as unknown as ResizeObserverEntry], this);
    });
  }
  unobserve(): void {
    // Sized once on observe; nothing to stop.
  }
  disconnect(): void {
    // See unobserve.
  }
}

beforeEach(() => {
  canvas = { images: [], fills: 0 };
  bitmaps = [];
  vi.stubGlobal("ResizeObserver", SizedResizeObserver);
  vi.stubGlobal("requestAnimationFrame", () => 0);
  vi.stubGlobal("cancelAnimationFrame", () => undefined);
  vi.stubGlobal(
    "createImageBitmap",
    vi.fn(async () => {
      const bitmap = { width: 100, height: 50, close: vi.fn() };
      bitmaps.push(bitmap);
      return Promise.resolve(bitmap);
    }),
  );
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    fillStyle: "#000",
    globalAlpha: 1,
    fillRect: () => {
      canvas.fills++;
    },
    drawImage: (image: unknown) => {
      canvas.images.push(image);
    },
    setTransform: () => undefined,
    save: () => undefined,
    restore: () => undefined,
    translate: () => undefined,
    scale: () => undefined,
  } as unknown as RenderingContext);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("/settings/", () => {
  it("shows the data dir, logs dir and version", async () => {
    await bootApp("/settings/", handLayoutHandlers());
    expect(await screen.findByText("/home/pilot/.local/share/wolluf")).toBeInTheDocument();
    expect(screen.getByText("/home/pilot/.local/share/wolluf/logs")).toBeInTheDocument();
    expect(screen.getByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByText("Language, folders and identity")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Choose which names are yours" })).toHaveAttribute("href", "/settings/identity");
  });

  it("switches language and persists the choice", async () => {
    await bootApp("/settings/", handLayoutHandlers());
    await userEvent.click(await screen.findByRole("radio", { name: "Español" }));
    await waitFor(() => {
      expect(i18n.language).toBe("es");
    });
    expect(window.localStorage.getItem(LANGUAGE_STORAGE_KEY)).toBe("es");
    expect(await screen.findByRole("heading", { name: "Ajustes" })).toBeInTheDocument();
  });

  it("keeps the segmented language control a radiogroup whose labels switch the language", async () => {
    await bootApp("/settings/", handLayoutHandlers());
    const group = await screen.findByRole("radiogroup", { name: "Language" });
    expect(within(group).getByRole("radio", { name: "English" })).toBeChecked();
    await userEvent.click(within(group).getByText("Español"));
    await waitFor(() => {
      expect(i18n.language).toBe("es");
    });
    expect(within(group).getByRole("radio", { name: "Español" })).toBeChecked();
    expect(await screen.findByText("Idioma, carpetas e identidad")).toBeInTheDocument();
  });

  it("opens the logs folder through app_open_logs_dir", async () => {
    const { calls } = await bootApp("/settings/", { ...handLayoutHandlers(), appOpenLogsDir: () => null });
    await userEvent.click(await screen.findByRole("button", { name: "Open logs folder" }));
    await waitFor(() => {
      expect(calls.map((c) => c.cmd)).toContain("app_open_logs_dir");
    });
  });
});

describe("/settings/ hand layout", () => {
  const NAMES = [
    "3 | 1+3 (right thumb)",
    "3+1 | 3 (left thumb)",
    "4 | 3",
    "3 | 4",
    "3 | 1 | 3 (either thumb)",
  ];

  it("lists the 7K layouts by readable name with the current one checked", async () => {
    const { calls } = await bootApp("/settings/", handLayoutHandlers("k7.43"));
    const group = await screen.findByRole("radiogroup", { name: "Hand layout (7K)" });
    const radios = await within(group).findAllByRole("radio");
    expect(radios).toHaveLength(5);
    for (const name of NAMES) {
      expect(within(group).getByRole("radio", { name })).toBeInTheDocument();
    }
    expect(within(group).getByRole("radio", { name: "4 | 3" })).toBeChecked();
    expect(within(group).getByRole("radio", { name: "3 | 1+3 (right thumb)" })).not.toBeChecked();
    expect(calls.filter((c) => c.cmd === "settings_hand_layouts").map((c) => c.args)).toEqual([{ keymode: 7 }]);
    expect(calls.filter((c) => c.cmd === "settings_get_hand_layout").map((c) => c.args)).toEqual([{ keymode: 7 }]);
  });

  it("falls back to the id for a layout it has no name for", async () => {
    await bootApp("/settings/", {
      ...handLayoutHandlers("k7.future"),
      settingsHandLayouts: () => [...K7_LAYOUTS, { id: "k7.future", columns: K7_LAYOUTS[0]?.columns ?? [] }],
    });
    const group = await screen.findByRole("radiogroup", { name: "Hand layout (7K)" });
    expect(await within(group).findByRole("radio", { name: "k7.future" })).toBeChecked();
  });

  it("saves a choice through settings_set_hand_layout, refetches the preference and says it was saved", async () => {
    const { calls } = await bootApp("/settings/", handLayoutHandlers());
    const group = await screen.findByRole("radiogroup", { name: "Hand layout (7K)" });
    await userEvent.click(await within(group).findByRole("radio", { name: "3+1 | 3 (left thumb)" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Saved");
    expect(calls.filter((c) => c.cmd === "settings_set_hand_layout").map((c) => c.args)).toEqual([
      { keymode: 7, layoutId: "k7.313_left_thumb" },
    ]);
    await waitFor(() => {
      expect(calls.filter((c) => c.cmd === "settings_get_hand_layout")).toHaveLength(2);
    });
    expect(within(group).getByRole("radio", { name: "3+1 | 3 (left thumb)" })).toBeChecked();
    expect(within(group).getByRole("radio", { name: "3 | 1+3 (right thumb)" })).not.toBeChecked();
  });

  it("invalidates every hand-layout query so other screens redraw with the new split", async () => {
    const { queryClient } = await bootApp("/settings/", handLayoutHandlers());
    queryClient.setQueryData(["settings", "handLayout", 4], "k4.generic");
    const group = await screen.findByRole("radiogroup", { name: "Hand layout (7K)" });
    await userEvent.click(await within(group).findByRole("radio", { name: "3 | 4" }));
    await screen.findByRole("status");
    expect(queryClient.getQueryState(["settings", "handLayout", 4])?.isInvalidated).toBe(true);
  });

  it("keeps the previous choice and shows the error when saving fails", async () => {
    await bootApp("/settings/", {
      ...handLayoutHandlers(),
      settingsSetHandLayout: () => mockIpcError("INVALID_INPUT"),
    });
    const group = await screen.findByRole("radiogroup", { name: "Hand layout (7K)" });
    await userEvent.click(await within(group).findByRole("radio", { name: "3 | 4" }));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(within(group).getByRole("radio", { name: "3 | 1+3 (right thumb)" })).toBeChecked();
    expect(within(group).getByRole("radio", { name: "3 | 4" })).not.toBeChecked();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("shows the load error instead of the options when the layouts cannot be read", async () => {
    await bootApp("/settings/", {
      ...handLayoutHandlers(),
      settingsHandLayouts: () => mockIpcError("INTERNAL"),
    });
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup", { name: "Hand layout (7K)" })).not.toBeInTheDocument();
    expect(screen.getByText("0.1.0")).toBeInTheDocument();
  });

  it("speaks Spanish", async () => {
    await i18n.changeLanguage("es");
    await bootApp("/settings/", handLayoutHandlers());
    const group = await screen.findByRole("radiogroup", { name: "Distribución de manos (7K)" });
    expect(await within(group).findByRole("radio", { name: "3 | 1+3 (pulgar derecho)" })).toBeChecked();
    expect(within(group).getByRole("radio", { name: "3+1 | 3 (pulgar izquierdo)" })).toBeInTheDocument();
    expect(within(group).getByRole("radio", { name: "3 | 1 | 3 (cualquier pulgar)" })).toBeInTheDocument();
  });
});

describe("/settings/ default skin", () => {
  function skinEntry(folder: string, keymodes: number[]): SkinEntryDto {
    return { folder, name: folder, keymodes, iniMtime: "1" };
  }

  const LIST: SkinListDto = {
    skins: [skinEntry("Alpha4K", [4]), skinEntry("Pilot", [4, 7]), skinEntry("Zeta", [7])],
    current: "Pilot",
    maniaSpeed: null,
    maniaSpeedBpmScale: false,
  };

  /** Column 3's note comes from an image, so a skinned preview always issues a drawImage. */
  function skinDto(folder: string, keymode: number): SkinDto {
    return {
      folder,
      name: folder,
      version: 2.5,
      config: {
        keys: keymode,
        columnWidth: Array.from({ length: keymode }, () => 42),
        columnSpacing: Array.from({ length: keymode - 1 }, () => 0),
        columnLineWidth: Array.from({ length: keymode + 1 }, () => 2),
        hitPosition: 428,
        lightPosition: 413,
        widthForNoteHeightScale: 42,
        noteBodyStyle: "repeat_bottom",
        judgementLine: true,
        keysUnderNotes: false,
        upsideDown: false,
        barlineHeight: 1.2,
        colours: { column: [], columnLine: null, judgementLine: null, barline: null, hold: null },
      },
      effects: { scorePosition: 300, comboPosition: 111, lightingNWidth: [], lightingLWidth: [], lightColours: [], comboOverlap: 0 },
      images: [{ slot: "note.3", file: 0 }],
      files: [{ mime: "image/png", scale: 1, width: 100, height: 50, base64: "iVBORw0KGgo=" }],
      diagnostics: [],
    };
  }

  function skinHandlers(list: SkinListDto = LIST): CommandHandlers {
    return {
      ...handLayoutHandlers(),
      skinList: () => list,
      skinGet: (args) => skinDto(String(args["folder"]), Number(args["keymode"])),
    };
  }

  async function picker(): Promise<HTMLSelectElement> {
    const select = await screen.findByRole<HTMLSelectElement>("combobox", { name: "Default skin" });
    await waitFor(() => {
      expect(select).toBeEnabled();
    });
    return select;
  }

  function argsOf(calls: { cmd: string; args: unknown }[], cmd: string): unknown[] {
    return calls.filter((c) => c.cmd === cmd).map((c) => c.args);
  }

  it("lists the skins with a 7K block first, the rest marked defaults, and starts on the cfg skin", async () => {
    const { calls } = await bootApp("/settings/", skinHandlers());
    const select = await picker();
    expect([...select.options].map((o) => o.text)).toEqual(["None (procedural)", "Pilot", "Zeta", "Alpha4K (defaults)"]);
    expect(select).toHaveValue("Pilot");
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toEqual([{ folder: "Pilot", keymode: 7 }]);
    });
    expect(argsOf(calls, "label_pattern_examples")).toEqual([{ keymode: 7, layoutId: null }]);
    expect(await screen.findByRole("img", { name: "Pilot on a sample pattern" })).toBeInTheDocument();
    await waitFor(() => {
      expect(canvas.images).toContain(bitmaps[0]);
    });
  });

  it("starts on the default the Label screen stored", async () => {
    localStorage.setItem("wolluf.label.skin", JSON.stringify({ folder: "Zeta" }));
    const { calls } = await bootApp("/settings/", skinHandlers());
    expect(await picker()).toHaveValue("Zeta");
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toEqual([{ folder: "Zeta", keymode: 7 }]);
    });
  });

  it("stores a picked skin as the default and redraws the preview with it", async () => {
    const { calls } = await bootApp("/settings/", skinHandlers());
    await waitFor(() => {
      expect(canvas.images).toContain(bitmaps[0]);
    });
    await userEvent.selectOptions(await picker(), "Zeta");
    expect(localStorage.getItem("wolluf.label.skin")).toBe(JSON.stringify({ folder: "Zeta" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Saved");
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toEqual([
        { folder: "Pilot", keymode: 7 },
        { folder: "Zeta", keymode: 7 },
      ]);
    });
    expect(await screen.findByRole("img", { name: "Zeta on a sample pattern" })).toBeInTheDocument();
    await waitFor(() => {
      expect(canvas.images).toContain(bitmaps[1]);
    });
    expect(bitmaps[0]?.close).toHaveBeenCalledTimes(1);
  });

  it("stores None and draws the preview procedurally", async () => {
    await bootApp("/settings/", skinHandlers());
    await waitFor(() => {
      expect(canvas.images).toContain(bitmaps[0]);
    });
    const select = await picker();
    canvas.images.length = 0;
    canvas.fills = 0;
    await userEvent.selectOptions(select, "");
    expect(localStorage.getItem("wolluf.label.skin")).toBe(JSON.stringify({ folder: null }));
    expect(await screen.findByRole("img", { name: "None (procedural) on a sample pattern" })).toBeInTheDocument();
    await waitFor(() => {
      expect(canvas.fills).toBeGreaterThan(0);
    });
    expect(canvas.images).toEqual([]);
    expect(bitmaps[0]?.close).toHaveBeenCalledTimes(1);
  });

  it("draws procedurally without asking for a skin when the cfg names none", async () => {
    const { calls } = await bootApp("/settings/", skinHandlers({ ...LIST, current: null }));
    expect(await picker()).toHaveValue("");
    await waitFor(() => {
      expect(canvas.fills).toBeGreaterThan(0);
    });
    expect(argsOf(calls, "skin_get")).toEqual([]);
    expect(canvas.images).toEqual([]);
  });

  it("refetches the previewed skin when its skin.ini changes on disk, as the Label screen does", async () => {
    let list = LIST;
    const { calls, queryClient } = await bootApp("/settings/", { ...skinHandlers(), skinList: () => list });
    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toEqual([{ folder: "Pilot", keymode: 7 }]);
    });

    list = { ...LIST, skins: LIST.skins.map((s) => (s.folder === "Pilot" ? { ...s, iniMtime: "2" } : s)) };
    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: skinKeys.list() });
    });

    await waitFor(() => {
      expect(argsOf(calls, "skin_get")).toEqual([
        { folder: "Pilot", keymode: 7 },
        { folder: "Pilot", keymode: 7 },
      ]);
    });
  });

  it("shows a still placeholder instead of a loading pulse when the sample pattern cannot be read", async () => {
    await bootApp("/settings/", { ...skinHandlers(), labelPatternExamples: () => mockIpcError("INTERNAL") });
    const card = await screen.findByRole("region", { name: "Default skin" });
    expect(await within(card).findByText("No sample pattern to preview.")).toBeInTheDocument();
    expect(card.querySelector("[aria-busy='true']")).toBeNull();
    expect(card.querySelector(".motion-safe\\:animate-pulse")).toBeNull();
  });

  it("shows why the skins cannot be listed and keeps the rest of the page", async () => {
    await bootApp("/settings/", { ...skinHandlers(), skinList: () => mockIpcError("INTERNAL") });
    const card = await screen.findByRole("region", { name: "Default skin" });
    expect(await within(card).findByRole("alert")).toBeInTheDocument();
    expect(within(card).getByRole("combobox", { name: "Default skin" })).toBeDisabled();
    expect(screen.getByText("0.1.0")).toBeInTheDocument();
  });

  it("speaks Spanish", async () => {
    await i18n.changeLanguage("es");
    await bootApp("/settings/", skinHandlers());
    const select = await screen.findByRole<HTMLSelectElement>("combobox", { name: "Skin por defecto" });
    await waitFor(() => {
      expect(select).toBeEnabled();
    });
    expect([...select.options].map((o) => o.text)).toEqual([
      "Ninguna (procedural)",
      "Pilot",
      "Zeta",
      "Alpha4K (por defecto)",
    ]);
    expect(await screen.findByRole("img", { name: "Pilot en un patrón de ejemplo" })).toBeInTheDocument();
  });
});
