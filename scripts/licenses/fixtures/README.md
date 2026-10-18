# License notices fixtures

Each folder here is one case for the notices generator's tests (feature 010):
a fake lockfile and `node_modules`, fake crate folders, recorded `cargo tree`
and `cargo metadata` output, or a `licenses/` folder of records. A folder is
named for the rule it shows, and holds only what that case needs. A red fixture
shows one failure rule and nothing else.

No test that uses them needs `cargo`, the network or the real `node_modules`.
Every generator function takes its paths and the `cargo` runner as parameters;
the tests pass these in.

**Nothing here is formatted.** `.prettierignore` skips every fixture folder
(`fixtures/*/`) so a license text stays byte-exact; `fakeCargo.mjs` and this
README sit beside them and are checked as usual. Vitest finds no `*.test.*` file
in here. Fixture text avoids the spec-citation forms `scripts/citations.mjs`
looks for. Texts are LF with a final newline (`.gitattributes` makes every
checkout LF).

## Layout

```text
fixtures/
  fakeCargo.mjs            the stand-in cargo runner; exports PROJECT, TRIPLES, fakeCargo({ variant, root })
  project/                 the good case: a repository root
    package.json           name, version 0.1.0, repository.url "git+https://github.com/exodious/HoploDex.git"
    lock.json              the lockfile (named so Dependabot ignores it); locations "node_modules/<name>"
    LICENSE                a short stand-in for the GPLv3 text
    modules/<name>/        stands for node_modules/<name>: git ignores node_modules/ folders
    registry/<crate>-<version>/   stands for the cargo registry's crate folders
    licenses/              the good records: four JSON files, additional-permission.txt, texts/
  records/<case>/licenses/ a red case: the same shape as project/licenses/, only what the case needs
  cargo/                   recorded cargo output (below)
  openssl/<version>/include/openssl/opensslv.h
  vite-app/                the tiny app vitePlugin.test.ts builds (below)
```

A case's repository root is the folder above its `licenses/`; a record's
`licenses/texts/...` path resolves against it. A `package:<path>` text resolves
against the carrier's folder.

## npm packages (`project/lock.json`, `project/modules/`)

| Package | Shows |
| --- | --- |
| `shipped-mit` | a shipped package whose file is lower case (`license`) |
| `@scope/shipped-scoped` | a scoped package, `LICENSE.md` |
| `choice-pkg` | "Apache-2.0 OR MIT" with `LICENSE_APACHE-2.0` and `LICENSE_MIT` |
| `with-notice-pkg` | Apache-2.0 with `LICENSE` and `NOTICE.md` |
| `no-license-file` | no license file at all (an exception in the good records) |
| `spdx-only` | only `LICENSE.spdx`, SPDX metadata that is not a text (an exception) |
| `@fontsource-variable/example-font` | OFL-1.1, kind `font`; has a clarification |
| `@zxcvbn-ts/language-en` 4.1.1 | the carrier of "OpenSubtitles 2024 word frequencies": `NOTICE.md` and `THIRD_PARTY_LICENSES.md` |
| `@zxcvbn-ts/language-common` 4.1.3 | the carrier of "EFF Large Wordlist for Passphrases" (no NOTICE) |
| `dev-tool` (`dev`), `types-peer` (`devOptional`) | development-only: never shipped |

## Crates (`project/registry/`)

All are in `cargo/metadata.json`. The "in the tree" column is the good recorded
trees (`L` Linux, `W` Windows, `M` macOS).

| Crate | In the tree | Shows |
| --- | --- | --- |
| `single` 1.0.0 | L W M | MIT, `LICENSE` |
| `dual` 1.0.0 | L W M | "MIT OR Apache-2.0": `LICENSE-MIT`, `LICENSE-APACHE` |
| `both-in-one` 0.4.42 | L W M | two licenses in one `LICENSE.txt`, as `chrono` |
| `licenses-dir` 1.0.0 | L W M | texts under `LICENSES/` |
| `with-notice` 1.0.0 | L W M | Apache-2.0 with `NOTICE` and `AUTHORS` |
| `needs-exception` 1.0.0 | L W M | no license file: has an exception |
| `unplaceable` 1.0.0 | L W M | two files the matcher can't place: has a clarification |
| `pointer-and-license` 1.0.0 | L W M | `LICENSE-MIT` and a pointer-only `COPYING` (a few hundred bytes naming the licenses) |
| `baz-sys` 0.1.0 | L W M | builds C (an enabled `cc` build dependency) |
| `libsqlite3-sys` 0.38.1 | L W M | the carrier: `sqlcipher/LICENSE`, feature `bundled-sqlcipher` (in metadata), builds C |
| `linux-only` 1.0.0 | L | shipped on one triple |
| `windows-only` 1.0.0 | W | shipped on one triple |
| `prebuilt-lib-sys` 0.1.0 | W | carries `lib/prebuilt.lib` |
| `webview2-com-sys` 0.38.2 | W | carries `x64/WebView2LoaderStatic.lib`: the WebView2 loader's carrier |
| `libsqlite3-sys` 0.39.0 | variant | the carrier at another version |
| `extra` 1.0.0 | variant | a crate the `extra-crate` variant adds |
| `pointer-only` 1.0.0 | no | only a pointer `COPYING`: no text |
| `no-license` 1.0.0, `authors-only` 1.0.0 | no | no license file; `AUTHORS` alone is not one |
| `license-folder` 1.0.0 | no | a text under `license/` |
| `dotted` 1.0.0 | no | `LICENSE.Apache-2.0`, `LICENSE.MIT`, `NOTICE.txt` |
| `zlib-named` 1.0.0 | no | `LICENSE-ZLIB`, `LICENSE-APACHE`, `LICENSE-MIT` |
| `content-matching` 1.0.0 | no | "Unicode-3.0 OR Zlib OR BSD-3-Clause" in `LICENSE-1`, `-2`, `-3`: placed by content |
| `dual-by-content` 1.0.0 | no | MIT in `LICENSE`, Apache in `COPYING`: placed by content |
| `ships-license` 1.0.0 | no | ships `LICENSE`: a stale exception |
| `changed-license` 1.0.0 | no | declares "MIT OR Apache-2.0" where an exception recorded "MIT" |
| `zlib-only` 1.0.0 | `zlib-exception` variant | "MIT OR Apache-2.0 OR Zlib" with no license file: `records/exception-zlib` commits Zlib's text (and a statement that only names Apache-2.0 and MIT), so its notice is under Zlib; `records/exception-unplaced` commits a text of none, which fails |
| `quux` 2.0.0 | `gpl-crate` variant | "GPL-2.0-only" (below, "Recorded cargo output") |

Crate folders have a `Cargo.toml` naming the `license`. Texts are short but keep
each license's real opening lines, which is what the matcher reads. Every
license file is over 400 bytes, so a size test can't mistake it for a pointer.

## Recorded cargo output (`cargo/`)

`fakeCargo({ variant, root })` returns a runner `cargo(args) => stdout`, with
`args` the arguments after `cargo`. It answers:

- `fetch`: nothing.
- `metadata`: `metadata.json`, the whole workspace (it lists crates no triple
  ships, as the real output does: the over-count 010 research.md §5 describes). Its
  `resolve` is left in, as the real output has it, but nothing reads it: the
  enabled features come from the tree (below), since `resolve` unions the features
  of every target and of development dependencies.
- `tree` with `build` among its `-e`/`--edges`: `build-<triple>.txt`, which is
  `cargo tree -e build --target <triple> --invert cc --invert cmake --prefix none
  -f '{p}'`: `cc`, then the packages with a build dependency on it, flat. It
  includes `buildtool-sys`, which no triple ships, so a reader must keep only
  shipped crates.
- any other `tree`: `tree-<triple>.txt`, `cargo tree --locked -e normal,no-proc-macro
  --target <triple> --features custom-protocol --prefix none -f '{p} {f}'`: the
  `hoplodex v0.1.0 (<root>/src-tauri)` line first, one `name vX.Y.Z` per crate
  followed by its enabled features, comma-separated, when it has any
  (`libsqlite3-sys v0.38.1 bundled-sqlcipher,default`), and a repeated crate
  marked `(*)`.
- anything else throws.

`@ROOT@` in the recorded output becomes `root` (default: `project/`), so
`manifest_path`s point at `project/registry/<crate>-<version>/Cargo.toml`.

Variants (`cargo/variants/<name>/`, each replacing only the files it has):

| Variant | Replaces | Shows |
| --- | --- | --- |
| `carrier-0.39.0` | all trees, build lists and metadata | `libsqlite3-sys` at 0.39.0 |
| `no-sqlcipher-feature` | all trees | the carrier with only `default`, without `bundled-sqlcipher` |
| `extra-crate` | the Linux tree | `extra v1.0.0` added: its notice appears, and only on Linux |
| `path-package` | the Linux tree | a path package that isn't `hoplodex` (`rogue`): must fail |
| `zlib-exception` | the Linux tree and metadata | `zlib-only` 1.0.0 ("MIT OR Apache-2.0 OR Zlib", `registry/zlib-only-1.0.0`): an exception chooses the license by its text |
| `gpl-crate` | the Linux tree and metadata | `quux` 2.0.0 under GPL-2.0-only (`registry/quux-2.0.0`): not an allowed license |

## The tiny Vite app (`vite-app/`)

`vitePlugin.test.ts` builds these with Vite's `build()` and `hoplodexNotices()`.
The packages are `project/modules/{shipped-mit,@scope/shipped-scoped,dev-tool}`,
each with an `index.js` (`export const name = "<package>"`, which no license
rule reads); the test aliases each package name to its folder, since `modules/`
stands for `node_modules`. The marker below is passed to the plugin as a
withdrawn entry, not read from `policy.mjs`.

| Entry | Imports | Shows |
| --- | --- | --- |
| `main.js` (+ `worker.js`) | `shipped-mit`, `@scope/shipped-scoped`; the worker imports `shipped-mit` | the good build; reads `__HOPLODEX_PROGRAM__` |
| `main-dev.js` | `dev-tool` (marked `dev` in `project/lock.json`) | a bundled package outside the shipped set, in the main bundle |
| `main-dev-worker.js` (+ `dev-worker.js`) | the worker imports `dev-tool` | the same, only in the worker's bundle |
| `main-withdrawn.js` | `withdrawn.json` | the marker `fixture-withdrawn-marker-7Qz` in the main bundle |
| `main-withdrawn-worker.js` (+ `withdrawn-worker.js`) | the worker imports `withdrawn.json` | the marker only in the worker's bundle |

## Records (`project/licenses/`, `records/<case>/licenses/`)

The good records: six hand-recorded entries (EFF list, WebView2 loader, OpenSSL
with version `from-build`, OpenSubtitles, SQLCipher, SQLite), three exceptions
(`crate:needs-exception`, `npm:no-license-file`, `npm:spdx-only`), two
clarifications (`crate:unplaceable`, the example font, one `{ text }` and one
committed path), four native-code entries, and `texts/`, whose files each begin
with `Source: <url>`. The hand-recorded names are the six that policy.mjs's
entry-scoped exceptions use. Every JSON file is sorted by its first field
(code-unit order). A red case folder holds only the files it changes (the
others are absent, which reads as an empty list) and the texts it needs.

| Case | Shows |
| --- | --- |
| `hand-unsorted`, `exceptions-unsorted` | an unsorted file |
| `hand-unknown-field`, `clarify-unknown-field` | a misspelled or unknown key |
| `native-bad-what` | a `what` that is not one of the four |
| `missing-fields` | a hand-recorded entry with no `texts` or `reason`, and an exception with no `declared` |
| `hand-missing-text`, `exception-missing-text` | a listed text file that isn't there |
| `text-without-source` | a committed text with no `Source:` first line |
| `exception-stale` | an exception for `ships-license`, which now ships `LICENSE` |
| `exception-declared-changed` | an exception that recorded "MIT" for `changed-license` |
| `exception-unused`, `clarify-unused`, `native-unused` | an entry no target ships |
| `clarify-hash-changed` | a clarification whose recorded SHA-256 is of an older `LICENSE` |
| `clarify-missing-file` | a clarification naming a file the package doesn't have |
| `native-unlisted` | an empty native-code list, so every native crate is unlisted |
| `absent-files` | a `licenses/` folder with `native-code.json` only |

Cases that need a crate's declared license, features or build dependencies get
them from the test, which builds the check's input by hand from the registry
folders; `cargo/` serves the generator's own tests.

## OpenSSL headers

`openssl/3.5.4/include/openssl/opensslv.h` has `OPENSSL_VERSION_STR "3.5.4"`.
`openssl/1.1.1w/include/openssl/opensslv.h` is like the real 1.1.1 header: it
has `OPENSSL_VERSION_TEXT "OpenSSL 1.1.1w  11 Sep 2023"` and no
`OPENSSL_VERSION_STR`, which came with OpenSSL 3.
