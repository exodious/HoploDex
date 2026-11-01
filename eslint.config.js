import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";
import eslintConfigPrettier from "eslint-config-prettier";

export default tseslint.config(
  {
    ignores: ["dist", "build", "node_modules", "src-tauri/target", "e2e/wdio-logs", ".wdio"],
  },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      parserOptions: {
        // Type-aware rules (no-deprecated) need type information. The default
        // project covers the files no tsconfig includes: Vite configs, scripts
        // (012 research.md §7).
        projectService: {
          allowDefaultProject: [
            "vite.config.ts",
            "tools/*/vite.config.ts",
            "scripts/*.ts",
            "scripts/*/*.ts",
          ],
          maximumDefaultProjectFileMatchCount_THIS_WILL_SLOW_DOWN_LINTING: 50,
        },
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/no-deprecated": "error",
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
    },
  },
  eslintConfigPrettier,
);
