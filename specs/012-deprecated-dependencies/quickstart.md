# Quickstart: validating feature 012

Run each step in the dev container (`scripts/dev-container.sh …`), or directly if you are already inside it.

## Automated

1. **The check's rules** (Stories 1, 2 and 3), with fixture lockfiles and exceptions, plus the `deny.toml` and ESLint configuration guards:
   ```bash
   scripts/dev-container.sh npx vitest run scripts/check-npm-deprecated.test.ts scripts/deny-config.test.ts scripts/eslint-config.test.ts
   ```
   Expected: all pass.
2. **The real lockfile** (Story 5, SC-004, SC-005):
   ```bash
   scripts/dev-container.sh bash -c 'time npm run audit:deprecated'
   ```
   Expected: exit 0, no findings, the last line `npm deprecations ok: …`, and under 2 s of real time.
3. **The full audit** (needs network access):
   ```bash
   scripts/dev-container.sh npm run audit
   ```
   Expected: exit 0. `cargo deny` reports `advisories ok` with `yanked = "deny"`.
4. **Lint** (Story 4):
   ```bash
   scripts/dev-container.sh npm run lint
   ```
   Expected: exit 0. `grep -rn "no-deprecated --" src` lists only the three justified uses (research.md §7).
5. **The WebdriverIO update** (FR-017):
   ```bash
   scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e'
   ```
   Expected: the suite passes on Linux. Windows and macOS: not verified in this feature's lane.
6. **The Flatpak node sources match the lockfile**:
   ```bash
   scripts/dev-container.sh --flatpak npm run flatpak:sources
   ```
   Then `git status`. Expected: no diff once it is committed.

## Manual (best effort, before a release)

1. Read "Dependency audit" in `DEVELOPMENT.md`.
   Expected: the deprecation policy and the yanked-crate rule are in separate subsections, and each of the issue's five decisions is answered there.
2. In a scratch copy of the repository (never on `main`), add `"deprecated": "test"` to the `node_modules/react` entry of `package-lock.json`, then run `npm run audit:deprecated`.
   Expected: exit 1, with a message that `react` is a shipped direct dependency that has to be replaced.
