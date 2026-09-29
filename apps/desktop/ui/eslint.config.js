// @ts-check
import boundaries from "eslint-plugin-boundaries";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

// D14 (architecture §4, spec 005 "eslint"): who may import whom inside src/.
const elements = [
  { type: "app", pattern: "src/app" },
  { type: "ipc", pattern: "src/ipc" },
  { type: "shared", pattern: "src/shared" },
  { type: "feature", pattern: "src/features/*", capture: ["slice"] },
  { type: "route", pattern: "src/routes" },
];

const dependencyPolicies = [
  // Files of the same element (the same feature slice, shared/, ipc/…) may always import each other.
  { allow: { dependency: { relationship: { to: "internal" } } } },
  // Third-party packages are governed by no-restricted-imports, not by the element matrix.
  { allow: { to: { module: { origin: ["external", "core"] } } } },
  { from: { element: { type: "app" } }, allow: { to: { element: { type: "*" } } } },
  { from: { element: { type: "ipc" } }, allow: { to: { element: { type: "shared" } } } },
  { from: { element: { type: "shared" } }, allow: { to: { element: { type: "shared" } } } },
  {
    from: { element: { type: "feature" } },
    allow: {
      to: [
        { element: { type: ["shared", "ipc"] } },
        { element: { type: "feature", fileInternalPath: ["index.ts", "index.tsx"] } },
      ],
    },
  },
  {
    from: { element: { type: "route" } },
    allow: {
      to: [
        { element: { type: ["shared", "ipc"] } },
        { element: { type: "feature", fileInternalPath: ["index.ts", "index.tsx"] } },
      ],
    },
  },
  // The root route is the app shell (nav registry, header) and route tests boot the whole app; leaf routes stay
  // limited to the matrix above.
  {
    from: { element: { type: "route", fileInternalPath: ["__root.tsx", "*.test.tsx"] } },
    allow: { to: { element: { type: "app" } } },
  },
];

export default defineConfig(
  {
    ignores: ["dist/**", "node_modules/**", "src/routeTree.gen.ts", "src/ipc/bindings.ts", "lint-fixtures/**"],
  },
  {
    files: ["**/*.{ts,tsx}"],
    extends: [tseslint.configs.strictTypeChecked, tseslint.configs.stylisticTypeChecked],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: { "react-hooks": reactHooks, boundaries },
    settings: {
      "import/resolver": {
        typescript: {
          alwaysTryTypes: true,
          project: ["tsconfig.json", "lint-fixtures/tsconfig.json"],
          noWarnOnMultipleProjects: true,
        },
      },
      "boundaries/elements": elements,
      "boundaries/include": ["src/**/*", "lint-fixtures/src/**/*"],
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: true }],
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["@tauri-apps/*"],
              message: "Only src/ipc/ may talk to Tauri (D14). Use the generated bindings through ipc/client.ts.",
            },
          ],
        },
      ],
      "boundaries/dependencies": ["error", { default: "disallow", policies: dependencyPolicies }],
    },
  },
  {
    files: ["src/ipc/**", "lint-fixtures/src/ipc/**", "src/test/**"],
    rules: { "no-restricted-imports": "off" },
  },
  {
    files: ["**/*.js"],
    extends: [tseslint.configs.disableTypeChecked],
  },
);
