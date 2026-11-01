# Data Model: Deprecated Dependencies and Yanked Crates

No database or application data changes. This feature's data is the dependency tooling's: two read-only inputs and one hand-kept file.

## Inputs (read only)

- **`package-lock.json`** (lockfile v3), `packages` map: location → entry.
  - The root entry (`""`) has `dependencies` and `devDependencies`, by name.
  - Each other entry has `version`, optional `name` (for an alias), `dev` / `devOptional` (development-only, 010 research.md §6), and optional `deprecated` (the maintainer's message).
- **`src-tauri/deny.toml`**, `[advisories]`: `yanked`, `unmaintained`, `unsound`, `ignore` (the yanked-crate guard reads these).

## Deprecation finding (computed)

| Field | From | Notes |
|---|---|---|
| `name` | entry `name`, or the location after its last `node_modules/` | as `npmPackages()` gives it |
| `version` | entry `version` | |
| `location` | the `packages` key | for example `node_modules/@wdio/config/node_modules/glob` |
| `scope` | `shipped`, or `dev` when `dev`/`devOptional` | 012 FR-004 |
| `direct` | location is `node_modules/<name>`, and `<name>` is in the root's `dependencies` (shipped) or `devDependencies` (dev) | 012 FR-005 |
| `message` | entry `deprecated`, a non-empty string | an empty or absent message means not deprecated |
| `outcome` | `fail` or `excepted` (by the index of one exception) | |

Rules, in order:

1. If `scope = shipped` and the dependency is direct, the finding **fails** with "replace it: a shipped direct dependency can't be excepted". Exceptions aren't consulted (FR-005, FR-012).
2. Otherwise, the first exception that matches is used: same `package`, `version` within `range`, same `scope`, and the exception itself is valid. A match makes the finding **excepted**. If the matching exception's review date has passed, the exception fails (FR-009). It still counts as matched, so it isn't also reported as stale.
3. If no exception matches, the finding **fails** (FR-005, FR-006).

## Deprecation exception (`audit-deprecated.json`, hand-kept)

```json
{
  "//": "Deprecated npm packages kept for now. See \"Dependency audit\" in DEVELOPMENT.md.",
  "exceptions": [
    {
      "package": "glob",
      "range": ">=10 <11",
      "scope": "dev",
      "reason": "why it can't be replaced yet",
      "via": "@wdio/config",
      "successor": "glob 13",
      "review": "2027-01-31"
    }
  ]
}
```

| Field | Type | Rule (a broken rule fails the audit, FR-011) |
|---|---|---|
| `package` | string | non-empty |
| `range` | string | readable by the range matcher (research.md §4) |
| `scope` | `"shipped"` \| `"dev"` | one of the two |
| `reason` | string | non-empty |
| `via` | string | non-empty: the package that brings it in, or `"package.json"` for a direct development dependency |
| `successor` | string \| `null` | `null` when none is known |
| `review` | string | a real calendar date, `YYYY-MM-DD` |

- No other keys are allowed.
- The top level holds only `"//"` and `exceptions`, and `exceptions` is an array.
- A missing file counts as no exceptions.
- An unreadable file fails the audit.

Lifecycle:

- An exception is **valid** while `review` ≥ today (UTC) and it matches at least one finding.
- Once `review` < today, it is **expired**, and the audit fails until someone replaces the package, or reviews it and records a new date (FR-009).
- An exception that matches no finding is **stale**, and the audit fails with "remove the exception" (FR-010).

## Yanked crate

- There is no exception entity.
- `deny.toml` must keep `yanked = "deny"`.
- Its `[advisories] ignore` may hold only advisory IDs (`RUSTSEC-`, `GHSA-` or `CVE-`) and never a crate spec (012 FR-014).
