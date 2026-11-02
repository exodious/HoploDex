/**
 * Whether timing budgets are checked: everywhere but under CI (`CI=true`, which
 * GitHub Actions sets), where shared runners' speed varies too much for a
 * budget to mean anything (#25). Only the time comparison is skipped; a test
 * still runs and makes every other check. The same rule as `budgets_checked()`
 * in `src-tauri/tests/support/mod.rs`; DEVELOPMENT.md, "Continuous integration".
 */
export function budgetsChecked(env: NodeJS.ProcessEnv = process.env): boolean {
  return env.CI !== "true";
}
