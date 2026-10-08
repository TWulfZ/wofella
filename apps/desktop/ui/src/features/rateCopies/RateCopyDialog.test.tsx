import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { jobTraySink, resetJobTray } from "@/features/jobs";
import type { JobDto, RateCopyPlanDto } from "@/ipc/bindings";
import { startEventBridge } from "@/ipc/eventBridge";
import { type CommandHandlers, emitMockEvent, mockCommands, mockIpcError } from "@/ipc/mocks";
import { i18n } from "@/shared/i18n";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { RateCopyAction } from "./RateCopyAction";
import { RateCopyDialog, type RateCopyTarget } from "./RateCopyDialog";

const MD5 = "a".repeat(32);
const JOB_ID = "01JRATECOPY";

const TARGET: RateCopyTarget = { md5: MD5, rateMilli: 1100, chartLabel: "xi - Blue Zenith [7K Insane]" };

const PLAN: RateCopyPlanDto = {
  previewId: "01JPREVIEW",
  md5: MD5,
  rateMilli: 1100,
  nightcore: false,
  folder: "/mnt/e/Games/osu!/Songs/292301 xi - Blue Zenith",
  osuFilename: "xi - Blue Zenith (Skystar) [7K Insane 1.1x (220bpm)].osu",
  version: "7K Insane 1.1x (220bpm)",
  audioFilename: "audio 1.10x.ogg",
  audioExists: false,
  osuExists: false,
  refusal: null,
};

function rateCopyJob(overrides: Partial<JobDto> = {}): JobDto {
  return {
    id: JOB_ID,
    kind: "rate_copy",
    status: "ok",
    started: "2026-10-08T10:00:00.000Z",
    ended: "2026-10-08T10:00:03.000Z",
    summary: {
      kind: "rate_copy",
      counters: {
        folder: PLAN.folder,
        osuFilename: PLAN.osuFilename,
        audioFilename: PLAN.audioFilename,
        osuWritten: true,
        audioWritten: true,
        audioReused: false,
        nextStep: "refresh_osu_then_sync",
        failedItems: 0,
      },
    },
    error: null,
    ...overrides,
  };
}

async function renderDialog(handlers: CommandHandlers = {}, target: RateCopyTarget = TARGET) {
  const onOpenChange = vi.fn();
  const calls = mockCommands({
    jobsList: () => [],
    rateCopyPlan: () => PLAN,
    rateCopyConfirm: () => JOB_ID,
    ...handlers,
  });
  const { queryClient } = renderWithRouter(<RateCopyDialog target={target} onOpenChange={onOpenChange} />);
  await startEventBridge(queryClient, jobTraySink);
  const dialog = await screen.findByRole("dialog", { name: "Generate a rate copy" });
  return { calls, dialog, onOpenChange };
}

function argsOf(calls: { cmd: string; args: Record<string, unknown> }[], cmd: string) {
  return calls.filter((c) => c.cmd === cmd).map((c) => c.args);
}

beforeEach(() => {
  resetJobTray();
});

describe("RateCopyDialog plan", () => {
  it("previews the new files when nothing exists yet", async () => {
    const { calls, dialog } = await renderDialog();
    expect(await within(dialog).findByText(PLAN.osuFilename)).toBeInTheDocument();
    expect(argsOf(calls, "rate_copy_plan")).toEqual([{ md5: MD5, rateMilli: 1100, nightcore: false }]);
    expect(dialog).toHaveTextContent("xi - Blue Zenith [7K Insane]");
    expect(dialog).toHaveTextContent("1.10x");
    expect(dialog).toHaveTextContent(PLAN.folder);
    expect(dialog).toHaveTextContent(PLAN.version);
    expect(within(dialog).getByText(PLAN.audioFilename)).toBeInTheDocument();
    expect(dialog).toHaveTextContent("Will be generated");
    expect(dialog).toHaveTextContent("wofella writes 2 new files into this set folder; it never overwrites or deletes anything");
    expect(within(dialog).getByRole("button", { name: "Generate" })).toBeEnabled();
  });

  it("says the audio at that rate is reused when it already exists", async () => {
    const { dialog } = await renderDialog({ rateCopyPlan: () => ({ ...PLAN, audioExists: true }) });
    expect(await within(dialog).findByText("Already exists, reused")).toBeInTheDocument();
    expect(dialog).not.toHaveTextContent("Will be generated");
    expect(dialog).toHaveTextContent("wofella writes 1 new file into this set folder; it never overwrites or deletes anything");
  });

  it("shows a refusal's localized reason with only a Close button", async () => {
    const { dialog, onOpenChange } = await renderDialog({
      rateCopyPlan: () => ({ ...PLAN, previewId: "", refusal: "keysounded" }),
    });
    expect(await within(dialog).findByText(/keysounds/)).toBeInTheDocument();
    expect(within(dialog).getAllByRole("button").map((b) => b.textContent)).toEqual(["Close"]);
    await userEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("has a reason for every refusal code", () => {
    const codes = [
      "unsupported_mode",
      "keysounded",
      "no_audio",
      "audio_missing",
      "malformed",
      "rate_out_of_range",
      "identity_rate",
      "already_rate_copy",
      "ln_heavy",
      "same_column_collision",
      "unsafe_name",
      "already_exists",
    ];
    for (const lng of ["en", "es"]) {
      for (const code of codes) {
        expect(i18n.exists(`rateCopy.refusal.${code}`, { lng }), `${lng} ${code}`).toBe(true);
      }
    }
  });

  it("speaks Spanish", async () => {
    await i18n.changeLanguage("es");
    mockCommands({ jobsList: () => [], rateCopyPlan: () => PLAN });
    renderWithRouter(<RateCopyDialog target={TARGET} onOpenChange={vi.fn()} />);
    const dialog = await screen.findByRole("dialog", { name: "Generar una copia con rate" });
    expect(await within(dialog).findByText("Se generará")).toBeInTheDocument();
    expect(within(dialog).getByRole("switch", { name: "Cambiar el tono (NC)" })).not.toBeChecked();
    expect(dialog).toHaveTextContent("Suena como el Nightcore de osu!; desactivado mantiene el tono, como DT.");
  });
});

describe("RateCopyDialog pitch", () => {
  const NC_PLAN: RateCopyPlanDto = {
    ...PLAN,
    previewId: "01JPREVIEWNC",
    nightcore: true,
    osuFilename: "xi - Blue Zenith (Skystar) [7K Insane 1.10x NC (220bpm)].osu",
    version: "7K Insane 1.10x NC (220bpm)",
    audioFilename: "audio 1.10x nc.ogg",
  };

  it("keeps the pitch by default and explains the NC option", async () => {
    const { dialog } = await renderDialog();
    const pitch = await within(dialog).findByRole("switch", { name: "Change pitch (NC)" });
    expect(pitch).not.toBeChecked();
    expect(pitch).toHaveAccessibleDescription("Sounds like osu! Nightcore; off keeps the pitch, like DT.");
  });

  it("re-plans with NC names when toggled and confirms the NC preview", async () => {
    const { calls, dialog } = await renderDialog({ rateCopyPlan: (args) => (args["nightcore"] === true ? NC_PLAN : PLAN) });
    expect(await within(dialog).findByText(PLAN.audioFilename)).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("switch", { name: "Change pitch (NC)" }));
    expect(await within(dialog).findByText(NC_PLAN.audioFilename)).toBeInTheDocument();
    expect(within(dialog).getByText(NC_PLAN.osuFilename)).toBeInTheDocument();
    expect(dialog).toHaveTextContent(NC_PLAN.version);
    expect(within(dialog).getByRole("switch", { name: "Change pitch (NC)" })).toBeChecked();
    expect(argsOf(calls, "rate_copy_plan")).toEqual([
      { md5: MD5, rateMilli: 1100, nightcore: false },
      { md5: MD5, rateMilli: 1100, nightcore: true },
    ]);
    await userEvent.click(within(dialog).getByRole("button", { name: "Generate" }));
    expect(argsOf(calls, "rate_copy_confirm")).toEqual([{ previewId: NC_PLAN.previewId }]);
    await waitFor(() => {
      expect(within(dialog).queryByRole("switch")).not.toBeInTheDocument();
    });
  });
});

describe("RateCopyDialog confirm", () => {
  it("confirms the preview, follows the job and ends with the F5 instruction", async () => {
    let jobs: JobDto[] = [];
    const { calls, dialog } = await renderDialog({ jobsList: () => jobs });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Generate" }));
    expect(argsOf(calls, "rate_copy_confirm")).toEqual([{ previewId: PLAN.previewId }]);

    await emitMockEvent("jobProgress", { jobId: JOB_ID, kind: "rate_copy", stage: "render_audio", done: 1, total: 2, etaMs: null });
    expect(await within(dialog).findByText("Rendering audio")).toBeInTheDocument();
    expect(within(dialog).getByRole("progressbar")).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Generate" })).not.toBeInTheDocument();

    await emitMockEvent("jobProgress", { jobId: JOB_ID, kind: "rate_copy", stage: "write", done: 2, total: 2, etaMs: null });
    expect(await within(dialog).findByText("Writing files")).toBeInTheDocument();

    jobs = [rateCopyJob()];
    await emitMockEvent("jobFinished", { jobId: JOB_ID, status: "ok", failedItems: 0 });
    expect(
      await within(dialog).findByText("Done. In osu!, press F5 in song select to see the new difficulty, then sync wofella."),
    ).toBeInTheDocument();
    expect(within(dialog).getByText(PLAN.osuFilename)).toBeInTheDocument();
    expect(within(dialog).getByText(PLAN.audioFilename)).toBeInTheDocument();
    expect(within(dialog).getAllByRole("button").map((b) => b.textContent)).toEqual(["Close"]);
  });

  it("lists a reused audio file as reused in the success state", async () => {
    let jobs: JobDto[] = [];
    const { dialog } = await renderDialog({ jobsList: () => jobs });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Generate" }));
    const done = rateCopyJob();
    if (done.summary?.kind !== "rate_copy") {
      throw new Error("unexpected summary");
    }
    jobs = [{ ...done, summary: { ...done.summary, counters: { ...done.summary.counters, audioWritten: false, audioReused: true } } }];
    await emitMockEvent("jobFinished", { jobId: JOB_ID, status: "ok", failedItems: 0 });
    const audio = await within(dialog).findByText(PLAN.audioFilename);
    expect(audio.closest("li")).toHaveTextContent("Reused");
  });

  it("shows a failed job's localized error", async () => {
    let jobs: JobDto[] = [];
    const { dialog } = await renderDialog({ jobsList: () => jobs });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Generate" }));
    jobs = [
      rateCopyJob({
        status: "failed",
        summary: null,
        error: { code: "CONFLICT", messageKey: "rate_copy.error.audio_target_unusable" },
      }),
    ];
    await emitMockEvent("jobFinished", { jobId: JOB_ID, status: "failed", failedItems: 0 });
    const alert = await within(dialog).findByRole("alert");
    expect(alert).toHaveTextContent("The rate copy failed");
    expect(alert).toHaveTextContent(/audio file name .* is taken/);
  });

  it("localizes a target that is not a regular file", async () => {
    let jobs: JobDto[] = [];
    const { dialog } = await renderDialog({ jobsList: () => jobs });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Generate" }));
    jobs = [rateCopyJob({ status: "failed", summary: null, error: { code: "CONFLICT", messageKey: "export.error.target_not_a_file" } })];
    await emitMockEvent("jobFinished", { jobId: JOB_ID, status: "failed", failedItems: 0 });
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(/not a regular file/);
  });

  it("shows a localized error when the preview expired before confirming", async () => {
    const { dialog } = await renderDialog({
      rateCopyConfirm: () => mockIpcError("NOT_FOUND", {}, { messageKey: "export.error.preview_unknown" }),
    });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Generate" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(/preview expired/);
  });

  it("clears a confirm error when the pitch switch re-plans", async () => {
    const { dialog } = await renderDialog({
      rateCopyConfirm: () => mockIpcError("NOT_FOUND", {}, { messageKey: "export.error.preview_unknown" }),
    });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Generate" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(/preview expired/);
    await userEvent.click(within(dialog).getByRole("switch", { name: "Change pitch (NC)" }));
    await waitFor(() => {
      expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument();
    });
  });

  it("names the refusal when confirm reports one", async () => {
    const { dialog } = await renderDialog({
      rateCopyConfirm: () => mockIpcError("INVALID_INPUT", { refusal: "ln_heavy" }, { messageKey: "rate_copy.error.refused" }),
    });
    await userEvent.click(await within(dialog).findByRole("button", { name: "Generate" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(/long notes/);
  });
});

describe("RateCopyAction", () => {
  it("opens the dialog at the picked rate, 1.10x by default", async () => {
    const calls = mockCommands({ jobsList: () => [], rateCopyPlan: (args) => ({ ...PLAN, rateMilli: Number(args["rateMilli"]) }) });
    renderWithRouter(<RateCopyAction md5={MD5} chartLabel="xi - Blue Zenith [7K Insane]" />);
    const rate = await screen.findByRole("combobox", { name: "Rate" });
    expect(rate).toHaveValue("1100");
    const options = within(rate).getAllByRole("option").map((o) => o.textContent);
    expect(options[0]).toBe("0.70x");
    expect(options.at(-1)).toBe("1.50x");
    expect(options).not.toContain("1.00x");
    await userEvent.selectOptions(rate, "1150");
    await userEvent.click(screen.getByRole("button", { name: "Generate rate copy" }));
    const dialog = await screen.findByRole("dialog", { name: "Generate a rate copy" });
    expect(dialog).toHaveTextContent("1.15x");
    await waitFor(() => {
      expect(argsOf(calls, "rate_copy_plan")).toEqual([{ md5: MD5, rateMilli: 1150, nightcore: false }]);
    });
  });
});
