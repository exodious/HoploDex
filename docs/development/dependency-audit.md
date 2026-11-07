# Dependency audit

_Part of the [development docs](../../DEVELOPMENT.md)._

```bash
npm run audit            # all six of the below
npm run audit:npm        # audit-ci over `npm audit`: dependencies and devDependencies
npm run audit:deprecated # deprecated npm packages, see below
npm run audit:cargo      # cargo-deny against the RustSec advisory database
npm run audit:openssl    # the Windows build's pinned OpenSSL against OpenSSL's advisories, see below
npm run audit:licenses   # the license audit, see licenses.md
npm run audit:webdriver  # no WebDriver code in the shipped app
```

Run it with the lint and test commands before every pull request. It fetches
the advisory databases, so it needs network access. The `Dependency audit`
workflow (`.github/workflows/audit.yml`) also runs it on every pull request to
`main`, Dependabot's included, and daily on `main`. A pull request that changes
only Markdown (`*.md`), or only Markdown and the other workflows' files
([Continuous integration](ci.md)), passes without the audit
running. `cargo-deny` comes with the
[development container](dev-container.md). On a host,
install it with `cargo install cargo-deny --locked`.

What fails the audit:

- **npm advisories**: any high or critical advisory, in devDependencies too.
  Moderate and low ones show up in `npm audit` but don't fail it.
- **npm deprecations**: a deprecated package version without a valid exception
  (always, for a shipped direct dependency). See
  [Deprecated npm packages](#deprecated-npm-packages).
- **Rust advisories**: any vulnerability advisory, whatever its severity (many
  RustSec advisories have no CVSS score), and any unsound notice.
- **Unmaintained crates**: any RustSec unmaintained notice without an
  exception. See [Unmaintained crates](#unmaintained-crates).
- **Yanked crates**: any yanked crate version, always. See
  [Yanked crates](#yanked-crates).
- **OpenSSL advisories**: any advisory of OpenSSL's, whatever its severity,
  that lists the OpenSSL version the Windows build's vcpkg pin resolves to;
  while that version's branch is out of support, any later advisory for the
  supported branches next to it, until checked; and a newer OpenSSL in vcpkg.
  See [The pinned OpenSSL](#the-pinned-openssl).

Advisories and deprecations appear all the time, so the audit can start
failing on a branch that changed no dependencies. There is no grace period: fix
the finding in its own commit.

To fix an advisory, update the dependency (`npm audit fix`, `npm update <pkg>`,
`cargo update -p <crate>`). If a parent pins a vulnerable transitive
dependency, try an npm `overrides` entry, and test the path that uses it.
Deprecations, unmaintained crates and yanked crates have their own sections
below.

Add an advisory exception only when there is no fix to take: no patched
release, or a parent that won't accept it. A critical advisory needs both, no patch
anywhere and an analysis. A high one needs a mitigation or a justification.
Record it next to the tool's config:

- **npm**: an `allowlist` entry in `audit-ci.jsonc`, scoped to the dependency
  path you analysed (`"GHSA-…|*parent>vulnerable-pkg*"`) rather than the bare
  advisory ID, so the same advisory arriving another way fails again.
- **Rust**: an `ignore` entry in `src-tauri/deny.toml`, with a `reason`.

Above the entry, write the advisory, why it can't be fixed yet, whether and how
it reaches HoploDex (shipped app or dev tooling only, and whether the
vulnerable code path is ever called), and a date to review it. Neither tool
expires entries, so that date is what brings it back, and the release review
(below) reads it by hand. Remove the entry once the fix is available.

## Deprecated npm packages

`npm run audit:deprecated` (`scripts/check-npm-deprecated.mjs`) reads
`package-lock.json` and `audit-deprecated.json` and needs no network. It
reports every lockfile location whose resolved version carries a deprecation
message. Only the version the lockfile resolves to counts: a deprecated old
major that nothing resolves to is not a finding. The record is
[012's spec](../../specs/012-deprecated-dependencies/spec.md).

- **Scope** is the license audit's definition ([License audit](licenses.md#license-audit)):
  a package marked `dev` or `devOptional` in the lockfile is development-only,
  anything else ships in the app.
- **Direct** means the package is in `package.json` and sits at its top-level
  location. A copy nested under another package is transitive.
- **A shipped direct dependency** that is deprecated fails whatever exceptions
  exist. Replace it. This is also the rule for adding a dependency: don't add
  one, or move one to a version, that is already deprecated.
- **A development-only package**, direct or transitive, fails without a dated
  exception.
- **A shipped transitive package** fails without a dated exception. Prefer
  updating the parent.

Add an exception only when the package can't be replaced yet. Each entry in
`audit-deprecated.json` has these fields, and the audit fails on a missing
field, an unknown key, or a value it can't read:

| Field       | Meaning                                                                          |
| ----------- | -------------------------------------------------------------------------------- |
| `package`   | The package name.                                                                |
| `range`     | The versions it covers (`>=10 <11`, `^10.2`, `*`, alternatives joined with `\|\|`). Empty means any. |
| `scope`     | `"shipped"` or `"dev"`. It must match the copy's scope.                          |
| `reason`    | Why it can't be replaced yet.                                                    |
| `via`       | The package that brings it in, or `"package.json"` for a direct dev dependency.  |
| `successor` | What replaces it, or `null` when none is known.                                  |
| `review`    | A date to look at it again, `YYYY-MM-DD`.                                        |

An example (not a current entry):

```json
{
  "package": "glob",
  "range": ">=10 <11",
  "scope": "dev",
  "reason": "the parent pins it and has no release that doesn't",
  "via": "@wdio/config",
  "successor": "glob 13",
  "review": "2027-01-31"
}
```

An exception matches by package, version range and scope, not by path, so a
dev exception never covers a copy that ships. It expires after its review
date (the date itself still passes), and then the audit fails until you replace
the package or record a new date with the reason. An exception that matches
nothing is stale and fails: remove it. There is no limit on how far ahead a
review date can be, so choose one you can defend.

## Unmaintained crates

A RustSec unmaintained notice fails `npm run audit:cargo` (`unmaintained =
"all"` in `src-tauri/deny.toml`). Its exceptions are `ignore` entries there,
with a `reason` and a comment above that gives the analysis and a review date,
as for an advisory. The comment also says whether the crate is direct (a
dependency of HoploDex itself) or indirect.

- **Indirect**: the comment names the parent that brings it in. Excepting it
  is allowed, once the parent can't be updated or replaced.
- **Direct**: HoploDex must not depend directly on an unmaintained crate. An
  exception is allowed only in a special circumstance, and the comment says
  that the crate is direct and why there is no alternative yet.

No tool tells direct from indirect, so reviewers check it, and the release
review does too.

## Yanked crates

A yanked crate version always fails `npm run audit:cargo` (`yanked = "deny"`),
whether it ships or is only used at build time. It is never excepted, and a
test (`scripts/deny-config.test.ts`) fails if `deny.toml` gains an entry that
would let one through. A crate yanked after it entered `Cargo.lock` fails a
branch that changed nothing: update it in its own commit, like any other
finding. Update the crate (`cargo update -p <crate>`), or move off
the parent that pins it.

cargo-deny reads yank state only from cargo's local index cache, and with a
complete `Cargo.lock` neither it nor `cargo fetch` refreshes an entry already
there. So `audit:cargo` first runs `cargo update --dry-run`, which refreshes
every locked crate's entry and leaves `Cargo.lock` as it is. Without it, a
cache that is weeks old would answer for the day it was written: a version
yanked since then would pass, and one yanked then and unyanked since would
fail (`zip` 7.2.0 and 8.6.0, yanked for a day in October 2026, failed the macOS
VM's audit this way).

## The pinned OpenSSL

The Windows build links OpenSSL statically from vcpkg, at the commit
`builtin-baseline` names in `scripts/windows/vcpkg/vcpkg.json` (#25).
Dependabot moves that pin by pull request (`.github/dependabot.yml`), but it
raises no security alerts for vcpkg, so `npm run audit:openssl`
(`scripts/check-openssl-advisories.mjs`) does that part:

- It reads the OpenSSL version the pin resolves to from vcpkg's
  `versions/baseline.json` at that commit. The manifest must leave the version
  to the baseline (no `version>=` or `overrides` for `openssl`), or the audit
  fails.
- It reads OpenSSL's own advisories, the CVE records OpenSSL publishes in
  [openssl/release-metadata](https://github.com/openssl/release-metadata)
  (`secjson/`), and fails on each one that lists that version as affected,
  naming it, its severity and its advisory. A record it can't read fails it
  too.
- It fails when vcpkg's `master` (its `versions/baseline.json` there)
  resolves OpenSSL to a newer version than the pin does. vcpkg's own
  port-version doesn't count. Take Dependabot's baseline update, or move
  `builtin-baseline` by hand.
- It warns when that version's branch is past its end of life in the same
  repository's `data.json`. OpenSSL lists only supported versions in a new
  advisory, so a later one may not name it. While the branch is out of
  support, the audit also fails on each advisory published on or after its
  end-of-life date that affects the nearest supported branch below it or the
  nearest above it (for 3.6, 3.5 and 4.0), which share its code. The message
  says to check whether the pinned version is affected. A `data.json` that is
  missing, can't be read, or has no end of life for that branch fails the
  audit, since it can't tell then.

It needs the network for all three, like the rest of the audit; without it, it
fails saying what it couldn't fetch. To fix a finding, move the pin to a vcpkg
commit whose OpenSSL is fixed (Dependabot's pull request, or `builtin-baseline`
by hand), which the Windows jobs then build and cache.

When the check of an advisory for a neighbouring branch shows the pinned
version isn't affected, record it in `audit-openssl.json`, as
`audit-deprecated.json` records a deprecation. Each entry in `exceptions` has
these fields, and the audit fails on a missing field, an unknown key, or a
value it can't read:

| Field    | Meaning                                                        |
| -------- | -------------------------------------------------------------- |
| `id`     | The advisory's CVE id, `CVE-2026-1234`.                        |
| `reason` | Why the pinned version isn't affected: what was checked.       |
| `review` | A date to look at it again, `YYYY-MM-DD`.                      |

An exception clears only that check, never an advisory that lists the pinned
version itself. It expires after its review date (the date itself still
passes), and then the audit fails until you check again and record a new date.
An exception that matches nothing the audit would fail on is stale and fails:
remove it. The notices need no
change: `licenses/hand-recorded.json` reads OpenSSL's version from the build's
headers.

## Before a release

The audit can't make these checks, so make them by hand with the license
checks:

- Review every npm deprecation exception in `audit-deprecated.json`, every
  OpenSSL exception in `audit-openssl.json` and every unmaintained-crate
  `ignore` entry in `src-tauri/deny.toml`, whatever its review date. For each, ask whether it can go now, and for an unmaintained
  crate whether it is direct or indirect. The advisory exceptions in
  `audit-ci.jsonc` and `deny.toml` aren't expired by a tool either: read their
  comment dates too.
- Check each direct dependency, npm and Rust, for an archived repository or a
  named successor.
- Check `NODE_VERSION` in the `Dockerfile`, and the npm it brings, against
  [Node's release schedule](https://nodejs.org/en/about/previous-releases).
