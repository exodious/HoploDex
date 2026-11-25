# Releasing HoploDex

How a release is made (#19). Pushing a `v*` tag starts the release workflow
(`.github/workflows/release.yml`), which checks the release, builds every
package, runs the release-time checks and opens a **draft** GitHub release.
A person checks the draft and publishes it. A final tag on an accepted
release candidate's commit builds nothing: the workflow promotes that
candidate's own files ([Release candidates](#release-candidates)). Which systems a release supports
is in the README's [Supported platforms](README.md#supported-platforms), and
nowhere else.

What a release publishes, all for x86-64 except the `.dmg`:

| Package | Built by |
|---|---|
| The Windows NSIS installer, `HoploDex-<version>-x86_64-setup.exe` (no MSI) | ci.yml's Packages (Windows) |
| The macOS `.dmg` for Apple silicon, `HoploDex-<version>-aarch64.dmg`, which carries the `.app` (no separate `.app`) | ci.yml's Packages (macOS) |
| The Linux `.deb` and `.rpm`, `HoploDex-<version>-x86_64.deb` and `.rpm`, built on `ubuntu-24.04`; the package they install is `hoplodex` | ci.yml's Packages (Linux) |
| The Flatpak, `HoploDex-<version>-x86_64.flatpak` | ci.yml's Flatpak |
| `SHA256SUMS`, every file's SHA-256 | release.yml |

Every file follows one pattern, `HoploDex-<version>-<x86_64|aarch64>`, then
`-setup.exe`, `.dmg`, `.deb`, `.rpm` or `.flatpak` (#209):
`scripts/check-package.mjs --rename` gives the Tauri packages their names in
the package jobs, and `scripts/flatpak/build.sh` names the Flatpak. Tauri
names the `.deb` and `.rpm` package after `productName` in kebab case, so
`scripts/build-appimage.sh` builds them with
`src-tauri/tauri.linux-packages.conf.json`, whose `productName` is `hoplodex`
and whose `conflicts`/`replaces` (`.deb`) and `obsoletes` (`.rpm`) make them
replace 0.1.0's `hoplo-dex` package.

Every package carries `NOTICES.txt` as a file of its own, beside the program
(`src-tauri/tauri.bundle.conf.json` for the Tauri bundles; the Flatpak's
manifest for the Flatpak), as well as inside it for About HoploDex.

**Signing.** No Apple Developer Program (no Developer ID signature, no
notarization) and no Windows code-signing certificate, for now. The macOS build
keeps its own ad-hoc signature, without which Apple silicon runs nothing:
`signingIdentity: "-"` in `src-tauri/tauri.conf.json` has Tauri sign the whole
`.app` ad-hoc (the linker's signature covers only the binary, which
`codesign --verify --deep --strict` refuses), with the hardened runtime on
(`hardenedRuntime: true`) and no entitlements. The macOS package job checks the
`.app` in the `.dmg` carries a valid one (`codesign --verify`). The Windows and
Linux packages and the `.flatpak` are unsigned. The release notes say how to
open an unsigned build.

## Release candidates

A tag with a pre-release suffix, such as `v0.1.1-rc.1`, makes a GitHub
pre-release, a release candidate. It carries the final version (`0.1.1`
everywhere, its files named `HoploDex-0.1.1-…`); only its tag has the suffix.
A release can be made two ways, and the release workflow picks by what is
published:

- **Through a release candidate**, when the packages need testing by hand
  before they ship: a change to packaging, the Flatpak, the database layout,
  or anything a person should try on each system first. The owner tests the
  candidate's files and **accepts it by publishing its pre-release**, which
  makes its files immutable and gives them GitHub's release attestation. Then
  `release.mjs promote` puts the final tag on the same commit, and the
  workflow makes the final release from that candidate's own files and
  `SHA256SUMS`, checked against its release, building nothing. What ships is
  byte for byte what was tested: the builds aren't reproducible, so a rebuild
  would ship files nobody tried.
- **Directly**, when no candidate is needed: a final tag on a commit with no
  published candidate of its version builds, tests and drafts the release, as
  a candidate's tag does.

A published candidate of the version on another commit than the final tag's
stops the workflow before anything runs: tag the accepted candidate's commit.
A candidate's draft that is never published (a rejected one) counts for
nothing. Everything below, the migration records and the AI security review
included, is done for the first tag of a version, a candidate or the final
itself:

- **The database layout freezes at the first release candidate.** Its
  migration records are merged before it (step 2), and from then on a released
  step never changes. A layout problem found in a release candidate's testing
  is fixed as a new step, in the next release candidate, whose `released.txt`
  line is merged before that candidate is tagged (step 2).
- **Nothing merges to `main` while a release candidate is under test.** The
  workflow takes a tag only on `main`'s head, as the AI security review's
  check (step 7) requires, so a merge during testing means a new release
  candidate, with its own review (`--after`) and its own testing.
- **The final release carries the release candidate's metainfo `<release>`
  date** (step 1), since its commit is the same.
- **Testers replace an earlier candidate by reinstalling it.** Every candidate
  of a version carries the same version, so a package manager won't install
  one over another; each candidate's notes say how
  (`apt install --reinstall`, `dnf reinstall`, `flatpak install --reinstall`,
  or the Windows installer's reinstall option), and name its commit.

## The release script

`scripts/release.mjs` runs the steps below that a command can do, in order, on
your own machine, and last creates and pushes the tag. Check out the commit
you will tag first:

```bash
git fetch origin
git switch --detach origin/main
node scripts/release.mjs all 0.1.0-rc.1 --dry-run   # print every command, the tag's included
node scripts/release.mjs all 0.1.0-rc.1             # run them, stopping at the first failure
node scripts/release.mjs checks 0.1.0               # or one step
```

Its steps are `version`, `records` (before a release candidate: it adds a line
for each step not yet listed and for the stored database unless it is listed,
and says so when there is nothing to add), `checks`,
`review`, `gate` and `tag`, and `promote` in place of `tag` for a final
release from an accepted candidate. `all` runs them in that order, all but `records`, whose lines
must already be on the commit (step 2). `--commit <sha>` names another commit
than `origin/main`'s head, and `--after <reviewed commit>` goes to the review.
`version`, `checks`, `all` and `promote` (whose audit reads the checkout's
lockfiles) refuse unless the working tree is clean and `HEAD` is that commit,
so what they test is what gets tagged. `tag`, and `all` before anything else, read the commit itself and refuse
what the workflow would: a tag that doesn't name its version, or one whose
commit has no stored database, or no `released.txt` line for it or for a step
in `MIGRATIONS`.
The tests run in the dev container when podman is there. It tags only when
every step before passed; it never makes a GitHub release. It leaves what the
release workflow runs to the workflow: the [dry run](#the-release-workflow) is
the check before a tag, on every OS. The version, its `<release>` entry and the checks a person makes (steps 1, 3 to 5)
are still yours.

For the final release from an accepted candidate, once its pre-release is
published and nothing has merged since:

```bash
git fetch origin
git switch --detach origin/main                    # the audit reads the checkout's lockfiles
node scripts/release.mjs promote 0.1.1 --dry-run   # print what it would run, the tag's included
node scripts/release.mjs promote 0.1.1             # or --rc 0.1.1-rc.2 to name the candidate
```

It finds the newest published `v0.1.1-rc.<n>` on `origin/main`'s head (or
`--commit`); `--rc` must name that one, since it is the one the workflow
promotes, and refuses an older candidate on the same commit. It refuses unless
the working tree is clean and `HEAD` is that commit, the candidate is an immutable pre-release built from that commit, its files
pass the checks the workflow's promote job makes (downloaded, they are exactly
a release's files, each uploaded by `github-actions[bot]`, from a release run
of the candidate's tag that passed on that commit, matching its `SHA256SUMS`,
and each passes `gh release verify-asset`), a dry run included, and the
workflow's tag check, `ai-security-review.mjs reviewed` and
`gate` (for any alert opened during testing) and `npm run audit` (the one
check whose result changes with time, not with the code) all pass. Then it
tags `v0.1.1` on that commit, its message naming the candidate, and pushes the
tag. The other steps ran on that commit for the candidate already. A final
release without a candidate is `release.mjs all 0.1.1`, like a candidate's:
its `tag` step refuses, before any step, a commit with a published candidate
of the version (that tag is `promote`'s, which checks the candidate's files
first) and a version whose published candidates are on other commits, as the
workflow would.

## Before the tag

Work through these on the commit you will tag, on `main`, for a version's
first tag, its first release candidate or the final itself (a later
candidate repeats them on its own commit, and a promoted final reuses the
accepted one's). A fix made along the way is a pull
request like any other; then start again from the new commit.

1. **The version.** Set it in `package.json` (and `package-lock.json`),
   `src-tauri/Cargo.toml` (and `Cargo.lock`) and `src-tauri/tauri.conf.json`. Add the
   release's `<release>` entry to `flatpak/io.github.exodious.HoploDex.metainfo.xml`,
   with its date and notes: the newest entry's version must equal the app's
   version (a test enforces it). A release candidate carries the version it
   is a candidate for; its tag adds the suffix. Its date is the final
   release's too, since the final tag reuses the candidate's commit.
2. **The migration records**, merged before the version's first tag, its
   first release candidate or a final with none (`src-tauri/src/db/migrations/README.md`,
   "At a release", and [Changing the data layout](docs/development/data-layout.md)):
   - add a line to `src-tauri/src/db/migrations/released.txt` for every step the
     release ships and for its stored database (`<name or path> <sha256> <release>`,
     in `MIGRATIONS`' order). Lines are never edited afterwards;
   - store the release's database as `src-tauri/tests/fixtures/releases/<release>.hoplodex`
     (`cargo run --example release_fixture` writes it). 0.1.0's is stored already;
   - run `release_upgrade_test.rs` and `released_migrations_test.rs`.

   `release.mjs records 0.1.0-rc.1` writes the lines into the working tree, with
   the app's version (`0.1.0`); commit them, run the tests with
   `release.mjs checks <version> --commit HEAD`, and merge them through a pull
   request. From that merge on the layout is frozen: a later release
   candidate's change to it is a new step, whose line `release.mjs records
   0.1.0-rc.2` adds the same way before that candidate. The workflow refuses
   any tag, a release candidate's included, whose commit has no `released.txt`
   line for its stored database or for a step in `MIGRATIONS`.
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
   - the metainfo's screenshot addresses are at the commit on `main` that last
     changed `flatpak/screenshots/`, and `release.mjs version` checks that
     commit is in the release's history and its images are the tree's. They
     are retaken only on purpose (`npm run screenshots:store`), in two pull
     requests, since `main` is squash-merged: the images, merged to `main`
     first, then the addresses moved to that commit on `main`;
   - review the pinned flatpak-builder-tools commit (`FLATPAK_BUILDER_TOOLS_COMMIT`
     in the `Dockerfile`) and Flathub's linter (`org.flatpak.Builder`, pinned
     as `linter_commit` in `scripts/flatpak/build.sh`). Move the linter's pin by
     hand to the current stable commit
     (`flatpak remote-info --log flathub app/org.flatpak.Builder/x86_64/stable`)
     in a pull request before the first release candidate, so the candidate
     and its pull request lint alike;
   - the GNOME runtime's end of life: #129 needs its decision in time.
6. **The local release-only tests.** `performance_test.rs` doesn't run in CI
   (its budgets need your machine): run it as [Test](docs/development/testing.md),
   "Release-only tests", says, or with `release.mjs checks`, which also runs
   the migration tests. Everything else the release needs tested, on every OS,
   the workflow's [dry run](#the-release-workflow) runs before the tag: E2E, the
   packages, both Flatpak checks, `npm run audit`, `import_largest_test` and
   the surface checks.
7. **The AI security review** (constitution, "Development Workflow & Quality
   Gates"). Run `/ai-security-review release <commit>` on the commit you will
   tag. It reviews the whole codebase against `docs/attack-surface.md` and
   uploads its findings to code scanning, on `main`. After fixes made in
   response to it, review the final commit with
   `/ai-security-review release <final commit> --after <reviewed commit>`, not
   a second whole review (`release.mjs review` starts it in Claude Code). Then
   run `node scripts/ai-security-review.mjs gate` (`release.mjs gate`):
   it must print PASS, with no critical or high code scanning, Dependabot or
   secret scanning alert open and no draft or triage advisory, except a draft
   whose every vulnerability lists this version as its patched version
   (`release.mjs gate` passes the release's x.y.z, without any `-rc.N`; run
   directly, the gate takes `tauri.conf.json`'s): that draft waits for this
   release, so the gate prints its GHSA id and lets it through. A triage
   advisory always blocks, whatever patched version its reporter gave. Each
   blocking alert is fixed, or dismissed with its reason.

   **How it is enforced:** the release workflow's first job refuses the tag
   unless code scanning holds the review's upload for exactly the tagged
   commit, on `main`, as the newest review there; the commit is still `main`'s
   head, so every tool's alerts there are its own; and no critical or
   high code scanning or Dependabot alert is open (`ai-security-review.mjs
   reviewed`). So tag `main`'s head, with nothing merged after the review. So a tag on any other
   commit, or one whose review left a blocking alert open, drafts no release.
   The workflow's token can't read secret scanning alerts or draft advisories,
   so that part of the gate is the `gate` run above.

## Tag

The `version tags` ruleset lets only the repository's admins create `v*`
tags. `release.mjs tag` (the last step of `all`) checks the workflow will
accept the commit, then tags it and pushes the tag. By hand, once the commit
carries the version and the records, that is:

```bash
node scripts/release.mjs tag-check 0.1.0-rc.1   # the workflow's tag check, with <commit> checked out
node scripts/ai-security-review.mjs reviewed --commit <commit>
git tag -a v0.1.0-rc.1 -m "HoploDex 0.1.0-rc.1" <commit>
git push origin refs/tags/v0.1.0-rc.1
```

A release candidate's tag is never moved, nor a final tag once its run has
drafted a release; immutable releases enforce it for anything published. If
the release needs another commit, it gets a new tag (the next release
candidate, say), after its own review (`--after`).

The one exception: **a final tag whose run drafted no release** (it stopped
before the draft, on an alert opened since `release.mjs promote`, say) may be
deleted and pushed again, on a new accepted candidate's commit or the same one,
by an admin, since the `version tags` ruleset lets only admins delete one. A
transient failure (the API, the network) needs no new tag: re-run the failed
job. A final draft that fails its own testing means a new patch version,
since its tag drafted a release.

## The release workflow

On the tag, `release.yml` runs:

- **Release review:** the tag names the app's version (`v<version>` or
  `v<version>-<suffix>`); the commit has that version's stored database and
  `released.txt` lines for it and every step in `MIGRATIONS`, for a release
  candidate too; and the AI security review
  covers the commit (step 7). It then chooses the path, and the run's summary
  says which: a final tag on a published candidate's commit promotes it, and
  every other tag builds.

On the build path:

- **CI**, ci.yml called with `release` set, from clean release builds: format,
  lint and the tests on every OS; E2E against the shipping profile
  (`HOPLODEX_E2E_PROFILE=release`) on all three, macOS's counting too; the
  packages, each checked by `scripts/check-package.mjs` (`NOTICES.txt` beside
  the program; in the `.deb`, apport's ignore entry; in the `.dmg`, the `.app`'s
  signature), the `.deb` and `.rpm` installed on clean Ubuntu and Fedora images with only what they declare (`scripts/check-linux-install.sh`), renamed to their release names and kept as the run's artifacts; and the Flatpak, with `test:flatpak` and then
  `test:flatpak -- --fresh-install` against the `.flatpak` the run built.
- **Release checks:** `npm run audit` and `import_largest_test`.
- **Surface checks** on each OS: the PDF surface check (on macOS also with
  WebKit's PDF HUD on), the window controls check and the drop check, its
  app part included on macOS and Windows
  ([PDF surface, window controls, drops](docs/development/surface-checks.md)).
- **Draft the release,** only when every job above passed: exactly the
  package jobs' `packages-*` artifacts, by name, one file each (a missing or
  extra one stops it), `SHA256SUMS`, the packages and the checksums kept together as the run's `release` artifact,
  and a draft release (a pre-release for a suffixed tag) with every file
  attached, the Flatpak included. Its notes are `.github/release-notes.md`,
  with the version and tag filled in (`{{app_version}}`, in the file names, is
  the version without a release candidate's suffix), the commit it was built
  from, and for a candidate how to reinstall it over an earlier one.

On the promote path, nothing is built, tested or surface-checked: the release
candidate's own release run did that. **Release checks** runs `npm run audit`
only, and **Promote the release candidate** checks the candidate's release is
published, immutable and a pre-release, with exactly a release's six files,
each uploaded by `github-actions[bot]` and passing
`gh release verify-asset`, from a release run of the candidate's tag that
passed on this commit, and matching its `SHA256SUMS`. Then it opens the draft
release with those files and that `SHA256SUMS` unchanged, its notes saying
which candidate it was promoted from. Any problem stops it before the draft.

A failed job can be re-run from the run's page; the draft is opened when they
all pass, and a re-run replaces the draft's files. The workflow never
publishes the draft or marks it latest. Nothing in the repository uploads to
Flathub or a store.

**A dry run.** Run the workflow by hand from the Actions tab (or
`gh workflow run release.yml --ref main`) to try every job but the draft on a
branch's head, before tagging: the tag check is skipped and the review check
reports without failing. GitHub runs it only from a branch that has the
workflow, so `main` once it has merged. After it, read the log of its "Check
the release's AI security review" step, not just the run's result: that step
doesn't fail a dry run, so a refusal there, such as a 403 listing Dependabot
alerts, shows only in its log.

**A rehearsal of promotion.**
`gh workflow run release.yml --ref main -f rehearse=v0.1.0` takes the promote
path against a published release's files instead of building: every check of
them but the pre-release and the file names (0.1.0's predate #209), and no
draft.

## Publish

1. Download the packages from the draft and try them: install each on a
   system it supports, open a throwaway collection, and check About HoploDex.
   A promoted final's files are the accepted candidate's, already tried:
   check its notes name that candidate and its files are the candidate's
   (`SHA256SUMS` is the same), with no install check.
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
3. Publish the draft once you approve it. Nothing else publishes it. A
   release candidate's pre-release is published only when it is accepted
   ([Release candidates](#release-candidates)); a rejected one stays a draft.

## After a release

A vulnerability in a supported version, one [SECURITY.md](SECURITY.md)'s
"Supported versions" lists, goes through a private security advisory, never a
public issue or pull request, until a fixed release is out. Until a version
is listed there, fixes go through ordinary public pull requests.
