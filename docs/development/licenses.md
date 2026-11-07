# Licenses

_Part of the [development docs](../../DEVELOPMENT.md)._

## License audit

HoploDex is released under GPL-3.0-only (`LICENSE`), so everything shipped
with it must be under a GPLv3-compatible license. `npm run audit:licenses`
checks the dependency trees and the third-party notices, and `npm run audit`
includes it. It works offline once the Cargo registry is fetched. Run it again
whenever you add or update a dependency, and before every release. The audit's
record, with what was checked, how and when, is
[010's research.md §1](../../specs/010-license-notices/research.md).

- **npm**: `scripts/check-npm-licenses.mjs` reads `package-lock.json` and
  checks every package that isn't development-only, since Vite bundles those
  into the app. devDependencies aren't distributed, so their licenses don't
  matter for the release. `node scripts/check-npm-licenses.mjs --all` lists
  them too, without failing on them.
- **Rust**: `cargo deny check licenses`, under `[licenses]` in
  `src-tauri/deny.toml`. It covers every crate for every target platform and
  feature, build and proc-macro crates included.
- **Notices**: `node scripts/licenses/notices.mjs --check --all-targets` builds
  the third-party notices for Linux, Windows and macOS from any machine and
  fails when one can't be made correctly. It can't read the Windows build's
  OpenSSL version; a Windows build does, from the headers named by the
  variables `libsqlite3-sys` reads (see [License notices](#license-notices)).

The first two use the same allow list of GPLv3-compatible SPDX licenses
(`ALLOWED` in `scripts/licenses/policy.mjs`, `allow` in `deny.toml`; a test
fails when the two differ). Before adding a license, check it against the FSF's
[list of GPL-compatible licenses](https://www.gnu.org/licenses/license-list.html).
A license that's only acceptable for particular packages goes in `EXCEPTIONS`
in `policy.mjs`, scoped to those packages or hand-recorded entries, with the
reason. Today they are OFL-1.1 for the `@fontsource` fonts, and the licenses
for data the strength hint's dictionary uses (below; the EFF, OpenSubtitles and
Moby entries are kept, though those lists no longer ship).

The Flatpak ships the same binary, built from the same lockfiles, and adds no
module of its own, so these checks already cover it; `scripts/flatpak/manifest.test.ts`
fails if its manifest gains a module or a source ([011's research.md §13](../../specs/011-flatpak/research.md)).

The passphrase strength hint is the `zxcvbn` crate's (`services/passphrase_dialog/strength.rs`),
worked out in the backend, since the passphrase is typed only in the native
dialog and the page never has it. The crate builds in six word lists, and its
MIT license doesn't speak for their data. Each ships on a ground (010 FR-015,
010 research.md §2, both amended 2026-11-04):

- **`passwords`** (Mark Burnett's ten million passwords) and **`female_names`,
  `male_names`, `surnames`** (the 1990 US Census lists): public domain, by the
  author's grant or by statute (US government works).
- **`english_wikipedia`, `us_tv_and_film`**: word-frequency lists counted
  mechanically from text (Wikipedia, and Wiktionary's subtitle frequency
  lists, both CC BY-SA 3.0 and the GFDL). They ship by the owner's exception
  to FR-015: in the US such a list is facts, which copyright doesn't protect
  (Feist), whatever license the counted text carries. Another list counted
  that way takes the same ground; any other data needs a found grant.

The page bundles no word list now, so `WITHDRAWN` in
`scripts/licenses/policy.mjs`, which the Vite plugin checks every emitted chunk
against, is empty.

The attributions these lists need (and SQLCipher's, SQLite's, OpenSSL's and the
WebView2 loader's) are hand-recorded entries in `licenses/hand-recorded.json`,
so they reach every build's notices.

The tools can't check some things, so check these by hand when they change and
before a release:

- **MPL-2.0 crates**: a file carrying MPL's Exhibit B notice ("Incompatible
  With Secondary Licenses") can't be combined with GPL code. When a new
  MPL-2.0 crate turns up (`cargo deny --manifest-path src-tauri/Cargo.toml list -l license`),
  grep its sources in `~/.cargo/registry/src/` for that phrase, leaving out
  the `LICENSE` file, which quotes it.
- **Exceptions and clarifications that are behind**: a version bump doesn't
  fail an exception or a clarification (see
  [License notices](#license-notices)), so before a release, list the entries
  in `licenses/exceptions.json` whose `checkedVersion` is behind the version
  that ships, and those in `licenses/clarify.json` whose `version` is behind,
  and compare the new version's license files with the committed text (an
  exception) or with the files the clarification names (a clarification: a
  file that changed fails its SHA-256, but a new file the clarification
  doesn't name does not).
- **New assets and data lists**: the type drawings
  (`src/features/browse/typeDrawings.ts`), the icon set
  (`src/components/Icon.tsx`), the chooser's catalogue plate
  (`src/features/databases/plate/`), and the program icon (`tools/icons/`,
  `src-tauri/icons/`) are the project's own work, under the project license.
  The plate's owl is drawn after the owl on the Athenian tetradrachm, a
  5th-century BC coin whose design is in the public domain. Record the source
  and license of any third-party artwork, font or data file, and of any word
  list added to the strength hint, before adding it. A list that needs a
  notice becomes a hand-recorded entry.
- **System libraries**: WebKitGTK, GTK and their stack on Linux (LGPL),
  WebView2 on Windows and WKWebView on macOS are system components, and the
  notices don't list them. No published bundle carries them (010 FR-017). A
  release publishes the Flatpak (its runtime comes from Flathub, with its
  source), the `.deb` and the `.rpm` (both use the system's libraries). The
  AppImage can still be built (`npm run bundle:linux`), but is not published:
  it carries those LGPL libraries, and publishing one first needs a decision
  on how it offers their source.

The checks that were manual are now automatic: the other word lists' sources
(above), C code built or a library linked inside a crate (the native-code
entries), and the OpenSSL version the Windows build links (the notices build
refuses anything older than 3, whose license isn't GPLv3-compatible).

Compatible doesn't mean there's nothing to do. MIT, BSD, Apache-2.0, ISC and
OFL all require their copyright and license notices to ship with the app. The
minified bundle and the stripped release binary don't carry them, so every
build produces a notices file; see [License notices](#license-notices).

## License notices

Every `npm run build` writes the third-party notices into `dist/`, for the
target the build is for (`TAURI_ENV_TARGET_TRIPLE` when Tauri runs it, so
`tauri build --target …` is followed, else the host's):

- `dist/notices.json`: the program block, the GPL-3.0 text, the additional
  permission, every shipped component with its license text. About HoploDex
  reads it.
- `dist/NOTICES.txt`: the same as plain text, for the bundles. Putting it next
  to the program in each bundle is the release process's (#19).

The build fails, naming what to fix, when a notice can't be made. The E2E
build uses the same `dist/`. `vite` (the dev server) serves `/notices.json`
instead and prints the failures as warnings. The formats are in
`specs/010-license-notices/contracts/notices.md`.

The generator is `scripts/licenses/notices.mjs`. Run it directly with:

```bash
node scripts/licenses/notices.mjs --check                      # every check for this build's target; writes nothing
node scripts/licenses/notices.mjs --check --target aarch64-apple-darwin   # the same for another target
node scripts/licenses/notices.mjs --out /tmp/notices           # write notices.json and NOTICES.txt there, for a look
node scripts/licenses/notices.mjs --check --all-targets        # Linux, Windows and macOS, and the entries no target uses (the audit)
```

It exits 1 on any failure, with one line per failure on stderr. Other lines
start `warning:`, such as an entry the target doesn't ship (not printed under
`--all-targets`, where an entry no target ships fails instead). It needs
`cargo` and the registry's sources: it runs `cargo fetch` for the target
first, as the `cargo build` that follows would. Under `--all-targets` it can't
read the OpenSSL version, since the variables that point a Windows build at
OpenSSL's headers exist only on a Windows build machine, so it checks that
entry's text and carrier only. On Windows the version is read from the
`openssl/opensslv.h` that `libsqlite3-sys`'s `build.rs` compiles against,
found as it finds it: each of `OPENSSL_LIB_DIR`, `OPENSSL_INCLUDE_DIR` and
`OPENSSL_DIR` is read as `<TARGET>_<name>` (for `x86_64-pc-windows-msvc`,
`X86_64_PC_WINDOWS_MSVC_OPENSSL_DIR`) first and then plain; the headers are in
`OPENSSL_INCLUDE_DIR` when it is set together with `OPENSSL_LIB_DIR` or
`OPENSSL_DIR`, else in `<OPENSSL_DIR>/include`, and without `OPENSSL_DIR` or
that pair the check fails, as the build does.

**What ships** is defined once, in `scripts/licenses/shipped.mjs`, and the
audit and the notices both use it:

- **npm**: every package in `package-lock.json` that is neither `dev` nor
  `devOptional`. The build also collects the package of every module in the
  main bundle (and any web worker's, of which there are none now), and fails when one isn't in that set.
  A package in the set that the bundler drops as unused stays listed.
- **Rust**: the crates `cargo tree -e normal,no-proc-macro --features custom-protocol --target <triple>`
  lists, `hoplodex` itself excluded. Build scripts' dependencies, procedural
  macros, dev dependencies and the `e2e` build's WebDriver plugin don't ship.
  A path crate other than `hoplodex` fails the check.
- **By hand**: the entries in `licenses/hand-recorded.json` that name the
  target's OS.

The components are the shipped packages and crates plus the hand-recorded
entries (Linux 348, Windows 272, macOS 265 today). Each is listed under one
license. For a choice (`MIT OR Apache-2.0`) it is the first allowed
alternative, in the order of `PREFERENCE` in `policy.mjs`, that the package
(or an exception's committed texts) has a text for. So `dunce`
(`CC0-1.0 OR MIT-0 OR Apache-2.0`) is listed under CC0-1.0 because that is the
text it ships, and `brotli-decompressor` (`BSD-3-Clause/MIT`) under
BSD-3-Clause. There is no fallback to a generic text, which would drop the
copyright lines.

### Answering a failure

The records are in `licenses/`. The JSON files are sorted by their first field,
and a test fails on an unsorted file or a misspelled key. A committed text
(under `licenses/texts/<name>/`) starts with a line `Source: <url>` that the
generator strips.

- **`no license text for MIT in its package`**: the published package leaves
  out its license file. Take the text from its repository at the commit the
  package records (`.cargo_vcs_info.json` for a crate), commit it, and add an
  exception to `licenses/exceptions.json`:

  ```json
  {
    "component": "crate:dlopen2",
    "checkedVersion": "0.8.2",
    "declared": "MIT",
    "texts": ["licenses/texts/dlopen2/LICENSE.txt"],
    "from": "https://raw.githubusercontent.com/OpenByteDev/dlopen2/cc80e4a0…/LICENSE",
    "reason": "the published crate leaves out its license file"
  }
  ```

  The text's first line is `Source:` and the same URL. Where several crates
  share one repository, they share one file (the `objc2` crates share
  `licenses/texts/objc2/LICENSE-MIT.txt`). Say in `reason` when the text comes
  from a later commit than the checked version, as the `objc2` crates' does.
  For a choice, the notice is under the first allowed alternative that one of
  the committed texts is a text of (the `zune` crates' repository holds only
  Zlib's, so they are listed under Zlib).

  An exception applies only to a version that ships no text of its own. When
  the package or crate starts shipping one, or declares another license, the
  check fails, saying to remove the exception.

- **`can't tell which of LICENSE, LICENSE.txt is the MIT text`**: a package
  with a choice of licenses has files the matcher can't place. Add a
  clarification to `licenses/clarify.json` naming the files to use, each with
  its SHA-256. A clarification fails when a file changes, so it is reviewed.
  It also corrects a notice: `@fontsource-variable/big-shoulders-display`'s
  `LICENSE` names "Google Inc.", but the font files and the upstream project
  name "The Big Shoulders Project Authors", so its clarification points to a
  committed `OFL.txt`:

  ```json
  {
    "component": "npm:@fontsource-variable/big-shoulders-display",
    "version": "5.3.0",
    "files": [{ "path": "licenses/texts/big-shoulders-display/OFL.txt", "sha256": "1b6bd753…" }],
    "reason": "the package's LICENSE names \"Google Inc.\" as the copyright holder; …"
  }
  ```

- **`hand-recorded "…"`** (its carrier isn't shipped, is another version or
  lacks the feature, or a text is missing): review what the entry describes
  and update its version, text or carrier. To add a component that no
  dependency tool lists (something compiled into the program or linked in, or
  data a list needs credited), add a hand-recorded entry to
  `licenses/hand-recorded.json`, with its license, the operating systems it
  ships on, its carrier (the crate or package that brings it, its version and
  the feature that does) and its text, either a file in the carrier
  (`package:<path>`) or a committed one. SQLCipher is the model:

  ```json
  {
    "name": "SQLCipher",
    "version": "4.14.0 community",
    "license": "BSD-3-Clause",
    "kind": "compiled-in",
    "os": ["linux", "macos", "windows"],
    "carrier": { "crate": "libsqlite3-sys", "version": "0.38.1", "feature": "bundled-sqlcipher" },
    "texts": ["package:sqlcipher/LICENSE"],
    "reason": "libsqlite3-sys compiles the SQLCipher source it carries into the program; …"
  }
  ```

  A license allowed only for such an entry (SQLite's `blessing`, CC-BY-4.0,
  ODC-By-1.0, `LicenseRef-public-domain`) goes in `policy.mjs`'s `EXCEPTIONS`,
  scoped to the entry.

- **`native code: crate … isn't in licenses/native-code.json`**: a shipped crate
  builds C or carries a prebuilt library. Review what it compiles or links, and
  add it to `licenses/native-code.json` with what it is: `hand-recorded:<name>`
  (and add that entry, as above), `own-code` (`objc2-exception-helper` compiles
  its own Objective-C file), `import-libraries` (`windows_x86_64_msvc`) or
  `system-library`.

- **`license: … isn't allowed`**: the license isn't on the allow list. Replace
  the dependency, or see [License audit](#license-audit) for an exception.
  The `objc2` crates are derived from Apple SDKs, and their repository says it
  is unclear whether distributing them is allowed. The owner accepted that on
  2026-10-18 (010 research.md §8), so they are used under their stated
  licenses.
- **`bundle: package … is bundled into the app but isn't in the audit's shipped
  set`**: the lockfile marks it `dev` or `devOptional`, yet Vite bundled it.
  Check its dependency type in `package.json`.
- **`the Windows build links OpenSSL 1.1.1w (headers from …)`**: build against
  OpenSSL 3. 1.1.x and older use the OpenSSL/SSLeay license, which isn't
  GPLv3-compatible. The message names the variable the headers came from; a
  `<TARGET>_OPENSSL_DIR` or an `OPENSSL_INCLUDE_DIR` set to an older OpenSSL
  overrides `OPENSSL_DIR`, so check them before `OPENSSL_DIR`.
- **`remove the exception`, `remove the clarification`, `remove the entry`**:
  the record is stale, either because the package now ships its own license
  file or declares another license, or (under `--all-targets`) because no
  target ships the component any more. Delete it and its committed text.

**A version bump fails a hand-recorded entry, not an exception.** An entry
describes what a carrier compiles or links in (SQLCipher's and SQLite's
versions, the loader's), and a new carrier version may change that, so the
entry must be looked at. An exception holds the project's license file,
which rarely changes between versions, and failing every Dependabot bump of
`objc2` would train people to update entries without looking. Its
`checkedVersion` shows how old the check is, and the pre-release list above
includes rechecking exceptions that are behind (010 research.md §8).
