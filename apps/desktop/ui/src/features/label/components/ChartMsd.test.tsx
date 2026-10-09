import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { ChartMsdDto } from "@/ipc/bindings";
import { mockCommands, mockIpcError } from "@/ipc/mocks";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { ChartMsd } from "./ChartMsd";

const MD5 = "a".repeat(32);
const SKILLSETS = ["overall", "stream", "jumpstream", "handstream", "stamina", "jackspeed", "chordjack", "technical"];

const RATED: ChartMsdDto = {
  md5: MD5,
  status: "rated",
  holdSharePermille: 40,
  calcVersion: 527,
  skillsets: SKILLSETS,
  rates: [
    { rateMilli: 900, centi: [2100, 2000, 1950, 1800, 1700, 1500, 1400, 1650] },
    { rateMilli: 1000, centi: [2345, 2210, 2105, 1990, 1888, 1601, 1507, 1802] },
    { rateMilli: 1100, centi: [2580, 2400, 2300, 2190, 2050, 1760, 1650, 1980] },
  ],
};

function unrated(status: ChartMsdDto["status"], holdSharePermille = 0): ChartMsdDto {
  return { ...RATED, status, holdSharePermille, rates: [] };
}

function renderMsd(dto: ChartMsdDto | (() => never)) {
  const calls = mockCommands({ chartMsd: typeof dto === "function" ? dto : () => dto });
  renderWithRouter(<ChartMsd md5={MD5} />);
  return calls;
}

function block(): HTMLElement {
  return screen.getByRole("region", { name: "Difficulty" });
}

describe("ChartMsd", () => {
  it("shows a rated chart's Overall at 1.0x, the calculator version and a collapsed rate table", async () => {
    const calls = renderMsd(RATED);
    expect(await screen.findByText("Rated")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "chart_msd")?.args).toEqual({ md5: MD5 });
    expect(within(block()).getByText("MinaCalc 527")).toBeInTheDocument();
    expect(within(block()).getByTestId("msd-overall")).toHaveTextContent("≈23.45");
    expect(within(block()).queryByRole("table")).not.toBeInTheDocument();

    const toggle = within(block()).getByRole("button", { name: "Show every rate" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(toggle);

    const table = within(block()).getByRole("table", { name: "MSD per rate and skillset" });
    expect(within(table).getAllByRole("columnheader").map((h) => h.textContent)).toEqual([
      "Rate",
      "Overall",
      "Stream",
      "Jumpstream",
      "Handstream",
      "Stamina",
      "JackSpeed",
      "Chordjack",
      "Technical",
    ]);
    const [, ...rows] = within(table).getAllByRole("row");
    expect(rows.map((r) => within(r).getByRole("rowheader").textContent)).toEqual(["0.9×", "1.0×", "1.1×"]);
    const nominal = within(table).getByRole("rowheader", { name: "1.0×" }).closest("tr");
    expect(nominal).not.toBeNull();
    expect(within(nominal ?? table).getAllByRole("cell").map((c) => c.textContent)).toEqual([
      "≈23.45",
      "≈22.10",
      "≈21.05",
      "≈19.90",
      "≈18.88",
      "≈16.01",
      "≈15.07",
      "≈18.02",
    ]);
    expect(within(block()).getByRole("button", { name: "Hide the rates" })).toHaveAttribute("aria-expanded", "true");
  });

  it("says an LN-heavy chart is not rated, with its long-note share, and shows no numbers", async () => {
    renderMsd(unrated("ln_heavy", 625));
    expect(await screen.findByText("LN-heavy: not rated")).toBeInTheDocument();
    expect(within(block()).getByText("62.5% long notes")).toBeInTheDocument();
    expect(within(block()).queryByTestId("msd-overall")).not.toBeInTheDocument();
    expect(within(block()).queryByRole("button", { name: "Show every rate" })).not.toBeInTheDocument();
  });

  it("marks a chart the calculator rejected", async () => {
    renderMsd(unrated("calc_rejected"));
    expect(await screen.findByText("Rejected")).toBeInTheDocument();
    expect(within(block()).queryByTestId("msd-overall")).not.toBeInTheDocument();
  });

  it("marks a chart not rated yet as pending", async () => {
    renderMsd(unrated("pending"));
    expect(await screen.findByText("Pending")).toBeInTheDocument();
    expect(within(block()).getByText("Index the library to rate this chart.")).toBeInTheDocument();
    expect(within(block()).queryByTestId("msd-overall")).not.toBeInTheDocument();
  });

  it("shows the error when chart_msd fails", async () => {
    renderMsd(() => mockIpcError("INTERNAL"));
    await waitFor(() => {
      expect(within(block()).getByRole("alert")).toBeInTheDocument();
    });
  });
});
