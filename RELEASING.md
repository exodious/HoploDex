# Releasing HoploDex

How a release is made (#19). Pushing a `v*` tag starts the release workflow
(`.github/workflows/release.yml`), which checks the release, builds every
package, runs the release-time checks and opens a **draft** GitHub release.
A person checks the draft and publishes it. Which systems a release supports
is in the README's [Supported platforms](README.md#supported-platforms), and
nowhere else.

What a release publishes, all for x86-64 except the `.dmg`:

| Package | Built by |
|---|---|
| The Windows NSIS installer, the setup `.exe` (no MSI) | ci.yml's Packages (Windows) |
| The macOS `.dmg` for Apple silicon, which carries the `.app` (no separate `.app`) | ci.yml's Packages (macOS) |
| The Linux `.deb` and `.rpm`, built on `ubuntu-24.04` | ci.yml's Packages (Linux) |
| The Flatpak, `HoploDex-<version>-x86_64.flatpak` | ci.yml's Flatpak |
| `SHA256SUMS`, every file's SHA-256 | release.yml |

Every package carries `NOTICES.txt` as a file of its own, beside the program
(`src-tauri/tauri.bundle.conf.json` for the Tauri bundles; the Flatpak's
manifest for the Flatpak), as well as inside it for About HoploDex.

**Signing.** No Apple Developer Program (no Developer ID signature, no
notarization) and no Windows code-signing certificate, for now. The macOS build
keeps its own ad-hoc signature, without which Apple silicon runs nothing:
`signingIdentity: "-"` in `src-tauri/tauri.conf.json` has Tauri sign the whole
`.app` ad-hoc (the linker's signature covers only the binary, which
`codesign --verify --deep --strict` refuses), without the hardened runtime,
which matters only for notarization. The macOS package job checks the `.app`
in the `.dmg` carries a valid one (`codesign --verify`). The Windows and Linux packages and the `.flatpak` are
unsigned. The release notes say how to open an unsigned build.

**Release candidates.** A tag with a pre-release suffix, such as
`v0.1.0-rc.1`, makes a GitHub pre-release. A release candidate doesn't count
as a release for the database's migrations: it adds no `released.txt` lines and
stores no release database, and its notes say a database it opened isn't
guaranteed to upgrade to the final release (the workflow writes that note).
Everything else below applies to it as to a final release, the AI security
review included.

## The release script

`scripts/release.mjs` runs the steps below that a command can do, in order, on
your own machine, and last creates and pushes the tag. Fetch first, so
`origin/main` is the commit you mean:

```bash
git fetch origin
node scripts/release.mjs all 0.1.0-rc.1 --dry-run   # print every command, the tag's included
node scripts/release.mjs all 0.1.0-rc.1             # run them, stopping at the first failure
node scripts/release.mjs checks 0.1.0               # or one step
```

Its steps are `version`, `records` (a final release only), `checks`,
`flatpak`, `review`, `gate` and `tag`; `--commit <sha>` names another commit
than `origin/main`'s head, and `--after <reviewed commit>` goes to the review.
The tests and the Flatpak run in the dev container when podman is there. It
tags only when every step before passed; it never makes a GitHub release. The
version, its `<release>` entry and the checks a person makes (steps 1, 3 to 5)
are still yours.

## Before the tag

Work through these on the commit you will tag, on `main`. A fix made along the
way is a pull request like any other; then start again from the new commit.

1. **The version.** Set it in `package.json` (and `package-lock.json`),
   `src-tauri/Cargo.toml` (and `Cargo.lock`) and `src-tauri/tauri.conf.json`. Add the
   release's `<release>` entry to `flatpak/io.github.exodious.HoploDex.metainfo.xml`,
   with its date and notes: the newest entry's version must equal the app's
   version (a test enforces it). A release candidate carries the version it
   is a candidate for; its tag adds the suffix.
2. **The migration records** (a final release only; `src-tauri/src/db/migrations/README.md`,
   "At a release", and [Changing the data layout](docs/development/data-layout.md)):
   - add a line to `src-tauri/src/db/migrations/released.txt` for every step the
     release ships and for its stored database (`<name or path> <sha256> <release>`,
     in `MIGRATIONS`' order). Lines are never edited afterwards;
   - store the release's database as `src-tauri/tests/fixtures/releases/<release>.hoplodex`
     (`cargo run --example release_fixture` writes it). 0.1.0's is stored already;
   - run `release_upgrade_test.rs` and `released_migrations_test.rs`.

   `release.mjs records` writes the lines and `checks` runs the tests; commit
   them through a pull request. The workflow refuses a final release's tag
   whose commit has no `released.txt` line for its stored database.
3. **The manual license checks** ([Licenses](docs/development/licenses.md), "License
   audit", and its "Exceptions and clarifications that are behind"):
   - grep each new MPL-2.0 crate for Exhibit B ("Incompatible With Secondary
     Licenses");
   - recheck each exception in `licenses/exceptions.json` whose `checkedVersion`
     is behind the shipped version, and each clarification in
     `licenses/clarify.json` whose `version` is behind;
   - record the source and license of any new asset or data list;
   - hold to the system-libraries rule: no published package carries an LGPL
     library (WebKitGTK, the GTK stack), which is why no AppImage is published;
   - `npm run audit:licenses` at the release commit (it includes
     `notices.mjs --check --all-targets`).
4. **The dependency audit's manual checks** ([Dependency audit](docs/development/dependency-audit.md),
   "Before a release"): every exception and `ignore` entry, archived or
   succeeded dependencies, and Node's release schedule. The Windows build links
   OpenSSL 3 statically from the vcpkg pinned in `scripts/windows/vcpkg/vcpkg.json`,
   and refuses anything older; `npm run audit:openssl` checks it.
5. **The Flatpak's per-release checks** ([Flatpak](docs/development/flatpak.md)):
   - the SDK extensions' branches (`rust-stable` and `node24` on the
     manifest's branch) are still stable on Flathub;
   - the metainfo's `<release>` entry (step 1);
   - the tag will carry `flatpak/screenshots/`, since the metainfo's screenshot
     addresses point at the final release's tag. They are retaken only on
     purpose (`npm run screenshots:store`);
   - review the pinned flatpak-builder-tools commit (`FLATPAK_BUILDER_TOOLS_COMMIT`
     in the `Dockerfile`) and Flathub's linter (`org.flatpak.Builder`, which
     `scripts/flatpak/build.sh` installs unpinned);
   - the GNOME runtime's end of life: #129 needs its decision in time.
6. **The local release-only tests.** `performance_test.rs` doesn't run in CI
   (its budgets need your machine): run it as [Test](docs/development/testing.md),
   "Release-only tests", says, or with `release.mjs checks`, which also runs
   the migration tests, `import_largest_test` and `npm run audit`. `release.mjs
   flatpak` builds the Flatpak and runs both of its checks.
7. **The AI security review** (constitution, "Development Workflow & Quality
   Gates"). Run `/ai-security-review release <commit>` on the commit you will
   tag. It reviews the whole codebase against `docs/attack-surface.md` and
   uploads its findings to code scanning, on `main`. After fixes made in
   response to it, review the final commit with
   `/ai-security-review release <final commit> --after <reviewed commit>`, not
   a second whole review (`release.mjs review` starts it in Claude Code). Then
   run `node scripts/ai-security-review.mjs gate` (`release.mjs gate`):
   it must print PASS, with no critical or high code scanning, Dependabot or
   secret scanning alert open and no draft or triage advisory. Each blocking
   alert is fixed, or dismissed with its reason.

   **How it is enforced:** the release workflow's first job refuses the tag
   unless code scanning holds the review's upload for exactly the tagged
   commit, on a branch, and no critical or high code scanning or Dependabot
   alert is open (`ai-security-review.mjs reviewed`). So a tag on any other
   commit, or one whose review left a blocking alert open, drafts no release.
   The workflow's token can't read secret scanning alerts or draft advisories,
   so that part of the gate is the `gate` run above.

## Tag

The `version tags` ruleset lets only the repository's admins create `v*`
tags. `release.mjs tag` (the last step of `all`) checks the workflow will
accept the commit, then tags it and pushes the tag. By hand, that is:

```bash
node scripts/ai-security-review.mjs reviewed --commit <commit>
git tag -a v0.1.0-rc.1 -m "HoploDex 0.1.0-rc.1" <commit>
git push origin refs/tags/v0.1.0-rc.1
```

A tag is never moved. If the release needs another commit, it gets a new tag
(the next release candidate, say), after its own review (`--after`).

## The release workflow

On the tag, `release.yml` runs:

- **Release review:** the tag names the app's version (`v<version>` or
  `v<version>-<suffix>`); a final release has its `released.txt` line; and the
  AI security review covers the commit (step 7).
- **CI**, ci.yml called with `release` set, from clean release builds: format,
  lint and the tests on every OS; E2E against the shipping profile
  (`HOPLODEX_E2E_PROFILE=release`) on all three, macOS's counting too; the
  packages, each checked by `scripts/check-package.mjs` (`NOTICES.txt` beside
  the program; in the `.deb`, apport's ignore entry; in the `.dmg`, the `.app`'s
  signature and a binary without the E2E-only WebDriver server) and kept as
  the run's artifacts; and the Flatpak, with `test:flatpak` and then
  `test:flatpak -- --fresh-install` against the `.flatpak` the run built.
- **Release checks:** `npm run audit` and `import_largest_test`.
- **Surface checks** on each OS: the PDF surface check (on macOS also with
  WebKit's PDF HUD on) and the window controls check
  ([PDF surface, window controls](docs/development/surface-checks.md)).
- **Draft the release,** only when every job above passed: `SHA256SUMS`, the
  packages and the checksums kept together as the run's `release` artifact,
  and a draft release (a pre-release for a suffixed tag) with every file
  attached, the Flatpak included. Its notes are `.github/release-notes.md`,
  with the version and tag filled in.

A failed job can be re-run from the run's page; the draft is opened when they
all pass, and a re-run replaces the draft's files. The workflow never
publishes the draft or marks it latest. Nothing in the repository uploads to
Flathub or a store.

**A dry run.** Run the workflow by hand from the Actions tab (or
`gh workflow run release.yml --ref main`) to try every job but the draft on a
branch's head, before tagging: the tag check is skipped and the review check
reports without failing. GitHub runs it only from a branch that has the
workflow, so `main` once it has merged.

## Publish

1. Download the packages from the draft and try them: install each on a
   system it supports, open a throwaway collection, and check About HoploDex.
2. Edit the draft's notes: add what changed. Keep the install instructions;
   they are the ones each release publishes:
   - **Windows:** the setup `.exe` isn't signed, so Windows may show
     "Windows protected your PC" (**More info**, then **Run anyway**); a
     computer in S mode has to leave S mode first.
   - **macOS:** it isn't notarized, so the first open needs **Open Anyway** in
     System Settings › Privacy & Security.
   - **The Flatpak:** `flatpak install --user HoploDex-<version>-x86_64.flatpak`,
     or the software centre. That adds Flathub if it isn't set up and fetches the
     GNOME runtime; a newer version installs the same way, over the old one,
     keeping everything; `flatpak update` updates the runtime but not a HoploDex
     installed from a file.
3. Publish the draft once you approve it. Nothing else publishes it.

## After a release

A vulnerability in a released version goes through a private security
advisory, never a public issue or pull request, until a fixed release is out
([SECURITY.md](SECURITY.md)).
