import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { bootApp } from "@/app/testing";
import type { HandLayoutDto } from "@/ipc/bindings";
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

function handLayoutHandlers(initial = "k7.313_right_thumb"): CommandHandlers {
  let current = initial;
  return {
    settingsHandLayouts: () => K7_LAYOUTS,
    settingsGetHandLayout: () => current,
    settingsSetHandLayout: (args) => {
      current = String(args["layoutId"]);
      return null;
    },
  };
}

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
