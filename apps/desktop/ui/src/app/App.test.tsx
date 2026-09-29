import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { mockCommands } from "@/ipc/mocks";
import { App } from "./App";
import { setupStatus } from "./testing";

describe("App", () => {
  it("renders the index route through the router, query client and i18n", async () => {
    mockCommands({ setupStatus: () => setupStatus(), jobsList: () => [] });
    render(<App />);
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent(/home|inicio/i);
  });
});
