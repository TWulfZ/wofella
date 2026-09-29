// @vitest-environment node
import { fileURLToPath } from "node:url";
import { ESLint } from "eslint";
import { describe, expect, it } from "vitest";

// Typed linting spins up a TypeScript program on first use, which is far slower than a unit test.
const LINT_TIMEOUT_MS = 60_000;

const uiRoot = fileURLToPath(new URL("../..", import.meta.url));
const fixture = (rel: string) => `lint-fixtures/src/${rel}`;

// `ignore: false` because the real config ignores lint-fixtures/ so that `pnpm lint` stays green.
const eslint = new ESLint({ cwd: uiRoot, ignore: false });

async function ruleIds(rel: string): Promise<string[]> {
  const results = await eslint.lintFiles([fixture(rel)]);
  const messages = results.flatMap((r) => r.messages);
  const fatal = messages.filter((m) => m.fatal === true);
  expect(fatal, `fatal lint errors in ${rel}`).toEqual([]);
  return messages.map((m) => m.ruleId ?? "");
}

const isBoundaries = (id: string) => id.startsWith("boundaries/");

describe("eslint rules fire on the D14 violation fixtures", () => {
  it(
    "a feature importing @tauri-apps/* hits no-restricted-imports",
    async () => {
      expect(await ruleIds("features/alpha/importsTauri.ts")).toContain("no-restricted-imports");
    },
    LINT_TIMEOUT_MS,
  );

  it(
    "a feature importing another feature's non-index file hits a boundaries rule",
    async () => {
      expect((await ruleIds("features/alpha/importsBetaInternal.ts")).some(isBoundaries)).toBe(true);
    },
    LINT_TIMEOUT_MS,
  );

  it(
    "shared importing a feature hits a boundaries rule",
    async () => {
      expect((await ruleIds("shared/importsFeature.ts")).some(isBoundaries)).toBe(true);
    },
    LINT_TIMEOUT_MS,
  );
});

// Controls prove the violations above come from the matrix, not from rules that reject everything.
describe("eslint rules stay quiet on the allowed fixtures", () => {
  it(
    "a feature may import another feature's index",
    async () => {
      const ids = await ruleIds("features/alpha/importsBetaIndex.ts");
      expect(ids.filter(isBoundaries)).toEqual([]);
    },
    LINT_TIMEOUT_MS,
  );

  it(
    "src/ipc may import @tauri-apps/*",
    async () => {
      expect(await ruleIds("ipc/importsTauri.ts")).not.toContain("no-restricted-imports");
    },
    LINT_TIMEOUT_MS,
  );

  it("the fixtures are excluded from the normal lint run", async () => {
    const normal = new ESLint({ cwd: uiRoot });
    expect(await normal.isPathIgnored(fixture("features/alpha/importsTauri.ts"))).toBe(true);
  });
});
