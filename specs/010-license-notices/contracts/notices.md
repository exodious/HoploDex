# Contract: The Notices Build and Its Files

**Feature**: [spec.md](../spec.md) | **Research**: [research.md](../research.md) §5–§10

## Commands

| Command | What it does | Fails (exit 1) when |
|---|---|---|
| `npm run build` | `tsc`, then `vite build`. The `hoplodexNotices()` Vite plugin writes `dist/notices.json` and `dist/NOTICES.txt` for the build's target | Any check below fails for that target. A bundled package isn't in the npm shipped set |
| `node scripts/licenses/notices.mjs --check [--target <triple>]` | Runs every check for one target (default: `TAURI_ENV_TARGET_TRIPLE`, else the host's) and writes nothing | Any check fails |
| `node scripts/licenses/notices.mjs --check --all-targets` | Runs the checks for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`, and the cross-target checks (an unused exception, clarification or native-code entry) | Any check fails |
| `node scripts/licenses/notices.mjs --out <dir> [--target <triple>]` | Writes `notices.json` and `NOTICES.txt` to `<dir>` without building, for inspection | As `--check` |
| `npm run audit:licenses` | As today, plus `notices.mjs --check --all-targets` | As today, or the above |

`--all-targets` can't read the OpenSSL version, since the variables that point
the Windows build at OpenSSL's headers exist only on a Windows build machine.
It checks the entry's text and carrier only. On Windows the header is found as
`libsqlite3-sys` 0.38.1's `build.rs` finds it (research.md §7): each of
`OPENSSL_LIB_DIR`, `OPENSSL_INCLUDE_DIR` and `OPENSSL_DIR` is read as
`<TARGET>_<name>` first (`X86_64_PC_WINDOWS_MSVC_OPENSSL_DIR`), then plain; with
both `OPENSSL_LIB_DIR` and `OPENSSL_INCLUDE_DIR` set, the headers are in
`OPENSSL_INCLUDE_DIR`; otherwise `OPENSSL_DIR` is required, and the headers are
in `OPENSSL_INCLUDE_DIR` if set, else `<OPENSSL_DIR>/include`.

**The command line** is run as `node scripts/licenses/notices.mjs` (the file
has no shebang: Vite's config bundler can't load one).
- `--check` or `--out <dir>` is required; `--all-targets` can't be combined
  with `--out` or `--target`; an unknown option, or one without its value,
  prints `notices: <what>` and the usage line, and exits 1.
- On success it prints one line to stdout, `notices: <triple> N components; 0
  failures` (with `--all-targets`: `notices: <triple> N, <triple> N, <triple>
  N components; 0 failures`) and exits 0.
- On failure it prints every failure, one per line, to stderr, then
  `notices: <triple> N components; M failures`, and exits 1.
- A warning (a record this target doesn't ship) prints to stderr as
  `warning: <message>` before the summary and never changes the exit code.
  `--all-targets` prints none: an entry no target uses fails there instead.
- `--out` writes the two files only when every check passed.

## Failure messages

Each names the component and says what to do. One line per failure, all
failures listed before exiting. A failed `npm run build` opens with a header
line, `notices: the build can't continue (N failures):`, then the failures, one
per line. The bundled-package message has a second form, as above, for a
package the lockfile doesn't list at all.

```text
crate dlopen2 0.8.2 (x86_64-unknown-linux-gnu): no license text for MIT in its package. Add an exception (licenses/exceptions.json) with the text from its source repository; see DEVELOPMENT.md, "License notices".
crate foo 1.2.0: can't tell which of LICENSE, LICENSE.txt is the MIT text. Add a clarification (licenses/clarify.json).
hand-recorded "SQLCipher": its carrier crate libsqlite3-sys is 0.39.0, recorded 0.38.1. Review the entry (its version, text and carrier) and update it.
hand-recorded "SQLCipher": carrier libsqlite3-sys is shipped without feature bundled-sqlcipher.
hand-recorded "SQLCipher": its carrier crate libsqlite3-sys isn't shipped on windows.
hand-recorded "OpenSSL": the Windows build links OpenSSL 1.1.1w (headers from OPENSSL_DIR); HoploDex needs OpenSSL 3 (its license is compatible with GPLv3, 1.1's isn't).
hand-recorded "OpenSSL": the OpenSSL version the Windows build links can't be read; set OPENSSL_DIR, or both OPENSSL_LIB_DIR and OPENSSL_INCLUDE_DIR (libsqlite3-sys's build.rs reads them, as X86_64_PC_WINDOWS_MSVC_<name> first).
hand-recorded "OpenSSL": C:\vcpkg\include\openssl\opensslv.h (opensslv.h, from OPENSSL_DIR) doesn't exist.
hand-recorded "OpenSSL": opensslv.h (C:\vcpkg\include\openssl\opensslv.h, from OPENSSL_DIR) names no OpenSSL version.
hand-recorded "SQLite": no target ships it; remove the entry (licenses/hand-recorded.json).
exception npm:react-remove-scroll-bar: the package now ships LICENSE; remove the exception.
exception npm:react-remove-scroll-bar: its folder /…/node_modules/react-remove-scroll-bar doesn't exist (run npm ci).
exception crate:foo: no target ships it; remove the exception (licenses/exceptions.json).
clarification crate:bar: LICENSE changed (sha256 …). Review it.
native code: crate baz-sys 0.1.0 (x86_64-pc-windows-msvc) builds C (cc) and isn't in licenses/native-code.json. Review what it compiles or links and record it.
native code: crate baz-sys: no target ships it; remove the entry (licenses/native-code.json).
bundle: package qux is bundled into the app but isn't in the audit's shipped set (package-lock.json marks it dev). Check its dependency type.
bundle: package qux is bundled into the app but isn't in the audit's shipped set (package-lock.json doesn't list it). Check its dependency type.
withdrawn: @zxcvbn-ts/language-en/dist/wikipedia.json is in the build (CC BY-SA 3.0 / GFDL, 010 research.md §2).
license: crate quux 2.0.0 is under GPL-2.0-only, which isn't allowed.
license: crate quux 2.0.0 declares no license expression.
crate quux 2.0.0 (x86_64-unknown-linux-gnu): its folder /…/quux-2.0.0 doesn't exist (cargo fetch should have fetched it).
npm quux 2.0.0: its folder /…/node_modules/quux doesn't exist (run npm ci).
crate quux 2.0.0 (x86_64-unknown-linux-gnu): no license text to show; a notice needs at least one. Check its record (licenses/clarify.json).
hand-recorded "SQLite": no license text to show; a notice needs at least one. Check its record (licenses/hand-recorded.json).
crates (x86_64-unknown-linux-gnu): cargo failed (<cargo's own message, on one line>).
crate quux 2.0.0 (x86_64-unknown-linux-gnu): not in cargo metadata's package list.
crate quux 2.0.0 (x86_64-unknown-linux-gnu): a path package ships in the app; only crates from a registry may.
npm packages: can't read /…/package-lock.json (<reason>).
npm package quux (node_modules/quux): package-lock.json records no version for it. Run npm install to repair the lockfile.
bundle: can't read /…/package-lock.json to check the bundled packages (<reason>).
LICENSE: can't be read (<reason>).
licenses/additional-permission.txt: can't be read (<reason>).
```

The record files add these, each naming the file and the entry (`<file>` is
one of the four JSON files; a `licenses/texts/…` file's own forms are in the
first lines above):

```text
<file>: the file is missing; every record file must exist (write an empty list as []).
<file>: can't be read as JSON (<parser's reason>).
<file>: must hold a list of entries.
<file>: an entry isn't an object.
<file>: entry "SQLite" appears twice.
<file>: entry "SQLite" is out of order after "OpenSSL"; sort the file by name.
<file>: entry "SQLite" has an unknown field "licence".
<file>: entry "SQLite" has no "reason".
<file>: entry "SQLite" has "texts", but it isn't a list.
<file>: entry "SQLite" has no texts; list at least one.
<file>: entry "crate:foo" has no files; list at least one.
hand-recorded.json: entry "SQLite" has no "os"; list at least one of linux, windows, macos.
hand-recorded.json: entry "SQLite" has os "freebsd"; each must be linux, windows or macos.
hand-recorded.json: entry "SQLite" has kind "bundled"; it must be compiled-in, linked or data.
hand-recorded.json: entry "SQLite" carrier must name exactly one of crate and npm.
hand-recorded.json: entry "SQLite" carrier has no "version".
hand-recorded.json: entry "SQLite" has an unknown carrier field "features".
clarify.json: entry "crate:foo" has an unknown file field "note".
clarify.json: entry "crate:foo" has a file that is neither { path, sha256 } nor { text }.
native-code.json: entry "libsqlite3-sys" has what "tool"; it must be hand-recorded:<entry name>, own-code, import-libraries or system-library.
native-code.json: entry "libsqlite3-sys" has what "hand-recorded:SQLCipher", but hand-recorded.json has no entry "SQLCipher".
```

The command line adds, each followed by the usage line:

```text
notices: --target needs a value
notices: --check takes no value
notices: unknown option --frobnicate
notices: give --check or --out <dir>
notices: --all-targets can't be combined with --out or --target
notices: can't tell the target (<reason>); give --target.
```

## `dist/notices.json`

```json
{
  "format": 1,
  "program": {
    "name": "HoploDex",
    "version": "0.1.0",
    "copyrightYears": "2026",
    "copyrightHolder": "exodious and the HoploDex contributors",
    "sourceUrl": "https://github.com/exodious/HoploDex"
  },
  "target": "x86_64-unknown-linux-gnu",
  "gpl": "                    GNU GENERAL PUBLIC LICENSE\n …",
  "additionalPermission": "Additional permission under GNU GPL version 3 section 7\n\n…",
  "components": [
    {
      "name": "@radix-ui/react-dialog",
      "version": "1.1.15",
      "kind": "npm",
      "license": "MIT",
      "texts": ["t12"],
      "source": "package"
    },
    {
      "name": "chrono",
      "version": "0.4.42",
      "kind": "crate",
      "license": "MIT",
      "declared": "MIT OR Apache-2.0",
      "texts": ["t40"],
      "source": "package"
    },
    {
      "name": "SQLCipher",
      "version": "4.14.0 community",
      "kind": "compiled-in",
      "license": "BSD-3-Clause",
      "texts": ["t7"],
      "source": "hand-recorded",
      "carrier": { "kind": "crate", "name": "libsqlite3-sys", "version": "0.38.1" }
    }
  ],
  "texts": {
    "t7": "Copyright (c) 2008-2020 Zetetic LLC\nAll rights reserved.\n…"
  }
}
```

- **Text ids** are opaque, stable within one document only.
- **The order**: `components` by `name` in lower case, then `version`. Within
  a component, the license text comes first, then NOTICE and attribution
  texts in the order the record gives.
- **A choice's `license`** (`declared` holds the whole expression; a slash,
  as in `MIT/Apache-2.0`, reads as `OR`) is the first allowed alternative, in
  the shared preference order, that the package has a text for (`findTextsAmong`),
  or, for an exception, that one of its committed texts is a text of
  (`placeTexts`). A package that ships only a less preferred alternative's
  text is used under that alternative; it fails only when no allowed
  alternative has a text. An exception applies only to a version that ships no
  text of its own: another version of the same crate (`zune-core` 0.5 beside
  0.4) uses its own files.
- **The JSON is stable**: keys in the order above, two-space indented, a final
  newline. So two builds of the same commit for the same target are
  byte-identical, and a diff of two releases' files reads as the change in
  what ships.

## `dist/NOTICES.txt`

UTF-8, LF line ends, 80 columns where the source text allows. In this order:

```text
HoploDex 0.1.0
Copyright (C) 2026 exodious and the HoploDex contributors

HoploDex is free software: you can redistribute it and/or modify it under the
terms of the GNU General Public License as published by the Free Software
Foundation, version 3 only. It is distributed in the hope that it will be
useful, but WITHOUT ANY WARRANTY, to the extent permitted by law; without even
the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
See the GNU General Public License below for more details.

Source code: https://github.com/exodious/HoploDex

<the additional permission>

This copy was built for x86_64-unknown-linux-gnu.

==============================================================================
Third-party components
==============================================================================

<for each component, in order:>
------------------------------------------------------------------------------
<name> <version>  (<license>[, used under <license> of <declared>])
------------------------------------------------------------------------------
<each of its texts, separated by a blank line>

==============================================================================
GNU General Public License, version 3
==============================================================================

<LICENSE>
```

Texts are repeated here for each component that uses them, not shared, so
any one component's notice can be read in isolation.

## Files under `licenses/` (committed)

| File | Holds | Shape |
|---|---|---|
| `licenses/additional-permission.txt` | The §7 permission (research.md §15) | Plain text |
| `licenses/hand-recorded.json` | Hand-recorded components | `[{ name, version, license, kind, os[], carrier: { crate\|npm, version, feature? }, texts[], reason }]` |
| `licenses/exceptions.json` | Notice exceptions | `[{ component, checkedVersion, declared, texts[], from, reason }]` |
| `licenses/clarify.json` | Clarifications | `[{ component, version, files: [{ path, sha256 } \| { text }], reason }]` |
| `licenses/native-code.json` | Native-code review | `[{ crate, what, note }]` |
| `licenses/texts/<component>/…` | Committed texts | As taken. Each has a first line `Source: <url>` that the generator strips |

All four JSON files must exist; an empty list is written `[]`. The JSON files
are sorted by their first field. A test fails on an unsorted file, an unknown
field or a missing required field, and on a hand-recorded `carrier` that
doesn't name exactly one of `crate` and `npm` (or has no `version`), so a hand
edit can't quietly misspell or drop a key. A hand-recorded `os` is a non-empty
list of `linux`, `windows` and `macos`, and its `kind` is `compiled-in`,
`linked` or `data`; `texts` (hand-recorded, exception) and `files`
(clarification) hold at least one item; a native-code `what: hand-recorded:<name>`
names an entry in `hand-recorded.json`. None of these drops out silently: each
is a failure.

## The Vite plugin (`scripts/licenses/vitePlugin.mjs`)

- **`vite build`**:
  - **During the build**: collects the `node_modules` package of every module
    id, in the main build and in the worker build (the plugin is in `plugins`
    and in `worker.plugins`).
  - **In `generateBundle`** of the main build, once both are known: checks
    them against the npm shipped set, runs the generator for the target, and
    emits `notices.json` and `NOTICES.txt` at the bundle's root, with fixed
    names.
  - **Withdrawn data**: every emitted chunk and text asset, the worker's
    included, is searched for `policy.mjs`'s withdrawn files' markers (today
    `wikipedia.json`'s encoded opening); a hit is a failure.
  - **On a failure**: it calls `this.error` with the failure list under a
    header line, which stops the build. The generator's warnings go to
    `this.warn`.
- **`vite` (the development server)**:
  - serves `GET /notices.json`, computed on the first request and cached until
    the lockfile changes;
  - prints failures as warnings and marks the document `development: true`.
- **`define` constants**: `__HOPLODEX_PROGRAM__`, the program block, in both
  build and serve. `src/vite-env.d.ts` declares it.
- **Vitest**: Vitest uses the same config, so it gets the `define` constants
  too. The plugin's `generateBundle` doesn't run under Vitest.
