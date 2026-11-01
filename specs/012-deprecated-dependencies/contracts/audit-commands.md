# Contract: the audit and lint commands this feature changes

## `npm run audit:deprecated`

`node scripts/check-npm-deprecated.mjs`. It reads `package-lock.json` and `audit-deprecated.json` from the repository root, makes no network access, and has no dependencies.

The output is one line per finding, then a summary:

```text
<scope>  <name>@<version>  <location>  <outcome>
    <deprecation message>
```

- `<scope>` is `shipped` or `dev`.
- `<outcome>` is `fails` or `excepted until <review>`.

Exit status:

- **0**, when no finding fails and no exception is expired, stale or malformed. The last line is `npm deprecations ok: …`.
- **1**, otherwise. Every problem is listed on stderr under a count line. The problems are:

| Case | Message names | Says |
|---|---|---|
| shipped direct dependency (FR-005) | `name@version`, location | replace it: a shipped direct dependency can't be excepted |
| no matching exception (FR-005, FR-006) | `name@version`, location, scope | replace it, or add an exception to `audit-deprecated.json` (DEVELOPMENT.md, "Dependency audit") |
| expired exception (FR-009) | its index, package, range, review date | review it: replace the package, or record a new date with the reason |
| stale exception (FR-010) | its index, package, range | remove the exception |
| malformed exception or file (FR-011) | its index (or the file), the field | what is wrong |

The module exports `checkDeprecated(lock, exceptionsFile, today)`, which returns `{ findings, problems }` for the tests. `today` is `YYYY-MM-DD`, and the CLI passes the UTC date.

## `npm run audit`

The chain gains `npm run audit:deprecated`, between `audit:npm` and `audit:cargo`. `.github/workflows/audit.yml` runs `npm run audit` and is unchanged.

## `npm run audit:cargo`

The command is unchanged. `src-tauri/deny.toml` gains `yanked = "deny"`, so a yanked crate version anywhere in the graph fails it. No `ignore` entry may let one pass: `scripts/deny-config.test.ts`, run by `npm test`, enforces that.

## `npm run lint`

The command is unchanged. `eslint.config.js` turns on `@typescript-eslint/no-deprecated` (error) with type-aware parsing, for `**/*.{ts,tsx}` only: plain-JavaScript scripts (`.mjs`) are not covered. A justified use carries this comment at its line:

```ts
// eslint-disable-next-line @typescript-eslint/no-deprecated -- <reason>
```
