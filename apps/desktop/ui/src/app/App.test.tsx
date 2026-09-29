import { render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { initI18n } from "@/shared/i18n";
import { App } from "./App";

describe("App", () => {
  beforeAll(async () => {
    await initI18n();
  });

  it("renders the index route through the router, query client and i18n", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent(/home|inicio/i);
  });
});
