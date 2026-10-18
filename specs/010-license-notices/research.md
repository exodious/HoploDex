# Research: License Notices in the App and the License Audit's Last Items

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Date**: 2026-10-18

Each section gives the decision, why, and what else was weighed. "Today"
describes the code at `b0cc3a6`. The dependency facts were gathered at
planning in the development container (`hoplodex-dev`), with `--locked`, from
the lockfiles at that commit. The word lists were checked against the
published tarballs of `@zxcvbn-ts/language-en` 4.1.1 and
`@zxcvbn-ts/language-common` 4.1.3 (their sha512 matches `package-lock.json`)
and the upstream repositories at those tags. §1 to §4 are the license audit's
record (FR-013, FR-014, FR-017). The implementation reruns every check and
updates the counts and the date (tasks.md); §1's tables are where the next
reviewer looks.

_Amended 2026-10-18 (010 T049): the implementation reran §1's checks on this
branch and rewrote its counts (the interface packages, the shipped crates, the
licenses that need a second look, the MPL-2.0 grep, the native-code list, the
fonts and the notices' own counts). §2's package versions and integrity
hashes were confirmed against `package-lock.json` and §3 gained the WebView2
SDK's version. The dated results are the same day as the planning's: both
ran on 2026-10-18._

## §1 The license audit's record (FR-013)

**Decision**: this section is the audit record the constitution's "Licensing"
section and #20 ask for. Each part of what ships has a row: what was checked,
how, the result and the date. The implementation's last audit task rewrites
the counts with the release build's figures and the date it ran.

"Shipped" means what a release build contains for one operating system (§5,
§6). Build scripts, procedural macros, development and test dependencies, and
the `e2e` build's WebDriver plugin don't ship.

| Part | What was checked | How | Result (2026-10-18) |
|---|---|---|---|
| **Interface packages** | Every package that is neither `dev` nor `devOptional` in `package-lock.json` (61 of its 827), against the allow list | `scripts/check-npm-licenses.mjs` (`npm run audit:licenses`) | All allowed: 55 MIT, 3 OFL-1.1 (only `@fontsource*`, the recorded exception), 1 0BSD, and 2 choices (`Apache-2.0 OR MIT`, `MIT OR Apache-2.0`). 55 of the 61 are in the built bundle (52 in the scripts, plus the 3 font packages' CSS and `woff2`). The other 6 are counted by the audit but never bundled: `loose-envify`, `js-tokens`, `detect-node-es`, `@radix-ui/react-arrow`, `@radix-ui/rect` and `@radix-ui/react-use-rect`, runtime packages that are removed as unused code (§6). The 4 type and CSS-type packages marked `devOptional` (`@types/prop-types`, `@types/react`, `@types/react-dom`, `csstype`) no longer count (T004). The build fails if a bundled package is outside the 61 (§6). 762 packages are `dev`. §6 |
| **Backend crates** | Every crate for every target and feature, build crates and procedural macros included | `cargo deny check licenses` (`src-tauri/deny.toml`) | All allowed. Shipped with the release feature (`custom-protocol`), counted with `cargo tree -e normal,no-proc-macro`: Linux 280, Windows 202, macOS 197 crates. 324 across the three OSes (a crate name and version counted once). The same counts as at planning, rerun on this branch; `Cargo.lock` is unchanged since `b0cc3a6` |
| **Licenses that need a second look** | Licenses other than MIT, Apache-2.0 and BSD among the shipped crates | `cargo tree … -f '{p} {l}'`, rerun for each OS | Unicode-3.0 (16 across the OSes, 15 on Windows and macOS: the `icu_*`/`zerovec` families, `unicode-ident`; one is `(MIT OR Apache-2.0) AND Unicode-3.0`). Zlib (16 across the OSes, 11 on Linux and Windows, 16 on macOS, mostly one choice among several; Zlib alone for `foldhash` and `zlib-rs`). BSL-1.0 (`ryu`, with Apache-2.0 as the other choice). MPL-2.0 (`option-ext` only). CC0-1.0 or MIT-0 (`dunce`). 0BSD (`adler2`). Apache-2.0 WITH LLVM-exception (`rustix`, `linux-raw-sys`). All GPLv3-compatible on the FSF's list. The macOS `objc2` crates (MIT, or Zlib, Apache-2.0 or MIT) are derived from Apple SDKs and their repository says it is unclear whether distributing them is allowed; the owner accepted that on 2026-10-18 (§8). No OpenSSL or SSLeay license anywhere in the tree |
| **MPL-2.0 Exhibit B** | Whether any MPL-2.0 file says "Incompatible With Secondary Licenses" | `grep -rli 'Incompatible With Secondary Licenses' --exclude='LICENSE*'` in the registry source of every MPL-2.0 crate in `cargo metadata` (rerun 2026-10-18, T049): `option-ext` 0.2.0 (the one shipped, on all three OSes), and `cssparser` 0.36.0, `cssparser-macros` 0.6.1, `dtoa-short` 0.3.5, `selectors` 0.36.1 (build-only, not shipped) | None, in any of the five |
| **C code compiled inside a crate** | Shipped crates whose build compiles bundled C, or links a library the crate carries | The shipped trees, each crate's `build.rs` and package contents (§7) | `libsqlite3-sys` 0.38.1 (`bundled-sqlcipher`) compiles SQLCipher 4.14.0 community (BSD-3-Clause, "Copyright (c) 2008-2020 Zetetic LLC", `sqlcipher/LICENSE` in the crate) on SQLite 3.51.3 (public domain, the blessing at `sqlcipher/sqlite3.c:64`). `libdbus-sys` 0.2.7 is built without `vendored`, so it links the system's `libdbus` and compiles nothing. On macOS, `objc2-exception-helper` compiles its own small Objective-C file, under its crate's license. On Windows, `webview2-com-sys` 0.38.2 links Microsoft's `WebView2LoaderStatic.lib` (§3). The guard (§7, `licenses/native-code.json`) lists the four crates the three real trees need: `libsqlite3-sys`, `objc2-exception-helper`, `webview2-com-sys` and `windows_x86_64_msvc`; no other shipped crate builds C or carries a prebuilt file, and `--check --all-targets` passes |
| **SQLCipher's crypto library** | Which library each OS's build links, and under what license | `libsqlite3-sys`'s `build.rs`; `scripts/windows/setup-system.ps1` | Linux: the system's OpenSSL `libcrypto`, dynamically, a system library. macOS: CommonCrypto (`SQLCIPHER_CRYPTO_CC`, the Security and CoreFoundation frameworks), a system library. Windows: OpenSSL from vcpkg (`x64-windows-static-md`), linked statically, so it ships inside the program. OpenSSL 3.x is Apache-2.0, which is compatible with GPLv3. 1.1.x and older use the OpenSSL/SSLeay license, which isn't, so the version decides. Today only `setup-system.ps1` checks the version (`OPENSSL_VERSION_MAJOR >= 3`), when a machine is set up. The notices build now reads the version from the `opensslv.h` that `libsqlite3-sys` 0.38.1's `build.rs` compiles against, found through the same environment variables in the same order (not from `OPENSSL_DIR` alone), and refuses anything older than 3 (§7). On the Windows test machine it read OpenSSL 3.6.5 from `opensslv.h` (T025), and refused a scratch copy of the headers that said 1.1.1w. `openssl-sys` isn't in the tree on any target |
| **Binary libraries a crate links in** | Shipped crates that carry and link a prebuilt library | Package contents (`*.lib`, `*.a`, `*.dll`, `*.dylib`, `*.so`) and `link` attributes | `webview2-com-sys` links `WebView2LoaderStatic.lib` statically under MSVC (`src/lib.rs`, `link(name = "WebView2LoaderStatic", kind = "static")`). It is Microsoft's, under the WebView2 SDK's BSD-3-Clause license, with no source published: `Microsoft.Web.WebView2` 1.0.3650.58, whose loader matches the one in `webview2-com-sys` 0.38.2 by SHA-256. §3 settles it. The `windows_*_msvc` crates carry import libraries generated by the `windows-rs` project, under that crate's own license, which name system DLLs |
| **System libraries** | What the program uses from the OS without shipping it | The bundle configuration, and §4 | Linux: WebKitGTK, GTK and their stack (LGPL), `libcrypto`, `libdbus`, the Secret Service. Windows: WebView2 (the Evergreen runtime). macOS: WKWebView, CommonCrypto, the Keychain. None ships in a published bundle (§4) |
| **Fonts** | The font files Vite emits | `src/main.tsx` imports; the `dist/assets/*.woff2` output (seven files, rechecked 2026-10-18 in this branch's build) | Atkinson Hyperlegible Mono and Next (latin, latin-ext) and Big Shoulders Display (latin, latin-ext, vietnamese): seven `woff2` files under OFL-1.1, shipped as separate font files (the recorded exception). The `@fontsource-variable/big-shoulders-display` 5.3.0 package's `LICENSE` names the copyright holder as "Google Inc.". The upstream project's `OFL.txt` names "The Big Shoulders Project Authors". The notice uses the text the font files themselves carry (§12) |
| **The strength hint's word lists** | Source and license of every list bundled into the strength worker | §2 | `wikipedia.json` no longer ships. The other seven the strength hint uses (`commonWords.json` and the six other lists that stated no source) ship, with their attributions in the notices |
| **What the notices list** | The components in each OS's `notices.json`, against the audit's shipped set | `node scripts/licenses/notices.mjs --check --all-targets` (rerun 2026-10-18, T049), and `scripts/licenses/notices.test.ts`'s real-tree tests on each OS | Linux 348, Windows 272, macOS 265 components, 0 failures. Linux: 280 crates, 61 packages (58 `npm`, 3 `font`) and 7 hand-recorded entries; Windows: 202, 61 and 9 (the OpenSSL and WebView2 loader entries are Windows-only); macOS: 197, 61 and 7. The records: 21 crates and 2 packages with an exception (their license text is committed), 1 clarification (Big Shoulders Display, §12), 9 hand-recorded entries, 4 native-code entries |
| **Data files in the program** | Data the backend embeds (`include_str!`) and the interface bundles | `grep -rn include_str src-tauri/src`; the specs' research | The cartridge catalog (`services/cartridges/catalog.tsv`): the project's own work (004 research.md, "catalog"; SAAMI and C.I.P. names used only for spelling). The built-in firearm types (`0003_seed_firearm_types.sql`) and registration form names (`services/registration.rs`, 005 research.md §7): the project's own short lists of names. The migrations and `preview/surface/frame_script.js`: the project's own code |
| **Artwork and icons** | The type drawings, the icon set, the catalogue plate, the program icon | DEVELOPMENT.md "Assets that aren't packages"; 005 research.md (the drawings) | The project's own work, under the project license. The plate's owl follows the Athenian tetradrachm's owl, a 5th-century BC design in the public domain |

**What the tools can't see now has a check** (§7): a shipped crate that starts
compiling or linking code it carries, or a hand-recorded component whose
carrying crate changes, fails `npm run audit:licenses` until someone reviews
it. The remaining manual checks are in DEVELOPMENT.md (FR-018): a new MPL-2.0
crate's Exhibit B, a new asset's source and license, and a new data list's
source.

## §2 The strength hint's word lists (FR-014, FR-015, FR-016)

**Decision**: `wikipedia.json` no longer ships. Every other list ships, with
these grounds and notices. FR-015 was widened at planning to cover
`lastnames.json` (public domain by statute) and `wordSequences.json` (written
by hand). That was the owner's decision; the spec's clarifications record it.

| List | What it is | Where it comes from | Ground (FR-015) | In the notices |
|---|---|---|---|---|
| `commonWords.json` (en) | 55,830 words, ranked by frequency | OpenSubtitles 2024 through OPUS (Helsinki-NLP), `CommonWordsGenerator` | The existing exception (003 clarification of 2026-09-26; DEVELOPMENT.md): ODC-BY, data looked up, not code combined. The FSF's list doesn't name ODC-BY | The package's `NOTICE.md` and `THIRD_PARTY_LICENSES.md`, word for word |
| `wikipedia.json` (en) | 29,782 words, ranked | Dropbox zxcvbn's `english_wikipedia` top 30,000, counted by `count_wikipedia.py` from the `enwiki-20151002` dump, then carried into zxcvbn-ts (a2ae466, d98c49f) | **None**. The 2015 text is under CC BY-SA 3.0 and the GFDL. Neither is GPLv3-compatible: CC designated GPLv3 compatible with BY-SA **4.0** only, one way, and the FSF doesn't list the GFDL. Reaching GPLv3 would need BY-SA 3.0's "later version" clause and then 4.0's compatibility, plus the argument that a frequency list isn't a derived work. FR-015 asks for a found grant, so it doesn't ship | (none) |
| `firstnames.json` (en) | 4,945 given names, alphabetical | `lists.ts`: `dominictarr/random-name`'s `first-names.txt` (MIT, "Copyright (c) 2012 'Dominic Tarr'"), which is Grady Ward's Moby `names-f.txt` (Project Gutenberg #3201) less one duplicate | (a) public-domain dedication: "Public Domain material by grant from the author, January, 2001" (Moby's readme) | random-name's MIT notice, and a credit to Moby Word Lists by Grady Ward (public domain) |
| `lastnames.json` (en) | 88,799 surnames, ranked | `lists.ts`: `arineng/arincli`'s `last-names.txt` (Apache-2.0), identical to Dropbox's `surnames.txt` from the 1990 US Census `dist.all.last` | (a) public domain by statute: 17 U.S.C. §105, a work of the US government | A credit to the U.S. Census Bureau, 1990 Census surname list (public domain) |
| `wordSequences.json` (en) | 14 short lists, 165 words: numbers, days, months, seasons, NATO alphabet, planets, zodiacs and the like | Written by hand by a zxcvbn-ts maintainer (8aba38e, PR #301, 2026-02-22) | (b) facts the project wrote itself, by hand, no third-party content | Covered by the package's MIT notice |
| `passwords.json` (common) | 49,233 passwords, ranked | `PasswordGenerator.ts` (MIT) over Mark Burnett's ten million passwords (xato.net, 2015-02-09) | (a) public-domain dedication: "So I built a data set of ten million usernames and passwords that I am releasing to the public domain." Short strings aren't copyrightable either | An acknowledgement of Mark Burnett's public-domain password corpus |
| `diceware.json` (common) | 7,776 words, alphabetical | Added by hand (5c5a74a, PR #179) from `dmuth/diceware`'s `wordlist-5-dice-eff.js` (Apache-2.0), which is the EFF Large Wordlist (Joseph Bonneau, EFF, 2016) with four edits | (a) CC BY 4.0 (eff.org/copyright), which the FSF lists as compatible with every GPL version. Apache-2.0 for the intermediate | The CC BY 4.0 attribution, required: title, author, source, license and the changes (§2 below) |
| `adjacencyGraphs.json` (common) | Six keyboard graphs | `KeyboardAdjacencyGraph.ts` (MIT) over the project's own layout drawings | (b) facts produced by the project's own script | Covered by the package's MIT notice |

**The diceware attribution** (CC BY 4.0 §3(a)), as the notices carry it:

> EFF Large Wordlist for Passphrases, by Joseph Bonneau for the Electronic
> Frontier Foundation (https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases),
> licensed under the Creative Commons Attribution 4.0 International License
> (https://creativecommons.org/licenses/by/4.0/). Changed: the dice numbers
> are removed, the hyphens are removed from "drop-down", "felt-tip" and
> "t-shirt", and "yo-yo" is replaced by "cheetah" (by github.com/dmuth/diceware,
> Apache-2.0).

_Confirmed 2026-10-18 (010 T049): `package-lock.json` still pins
`@zxcvbn-ts/language-en` 4.1.1
(`sha512-6UdzuBd3Uex8TKubohcn+uXRVAH34Zjs2eCfT4hQVo9zeTy7AkQRQfdV4OnHR5hQfW/XBrK/AGTZk7VBWh7wwQ==`)
and `@zxcvbn-ts/language-common` 4.1.3
(`sha512-g2bg4skmlJG9aJqC/nWziZzgYpSe/f3rKeGobrkdFiOrfZVFqqzUgC6BUQWuX54Sua9j0Jxxt+1bf4f/Mal1Kg==`),
the versions and hashes the word lists were checked against; the lockfile is
unchanged since `b0cc3a6`._

**Upstream** added `NOTICE.md` and `THIRD_PARTY_LICENSES.md` to
`language-common` on 2026-10-08 (PR #349), covering passwords and diceware. No
release has them yet. When one does, the hand-recorded entries' carrier check
(§7) fails on the version change, and the entries can switch to the package's
own text.

**Dropping the list from the bundle**: spreading the package's `dictionary`
reads every key, and the package's entry module imports all its lists, so
leaving a key out after the spread doesn't remove the list from the worker.
`estimate.ts` instead imports the kept lists from the packages' per-list
modules (`@zxcvbn-ts/language-en/dist/firstnames.json.mjs` and the like, with
`sideEffects: false`). It builds the same dictionary keys (`commonWords-en`,
`firstnames-en`, `lastnames-en`, the 14 `wordSequences` keys,
`passwords-common`, `diceware-common`), and keeps `translations` and
`adjacencyGraphs` from the entry modules, where the unused `dictionary` is
removed as unused code. An ambient `declare module` types the per-list
modules, which ship without declarations. Two checks prove it:
- a build check fails if the built worker contains `wikipedia.json`'s encoded
  opening;
- a test fails if `estimate.ts`'s dictionary has a `wikipedia` key.

The existing strength tests ("password" → score 0 with a suggestion, a
five-word phrase → 4, the `passwordpassword` warning in `databases.e2e.ts`)
keep passing, since those words come from `passwords.json` and the repeat
matcher (003 FR-003).

**Alternatives considered**:
- Replacing `wikipedia.json` with a public-domain list (Moby `common.txt`).
  Rejected: zxcvbn treats a word's position as its guess rank, Moby is
  alphabetical, and `commonWords.json` already ranks general English.
- Keeping it on the two-step BY-SA chain and the facts argument. Rejected by
  FR-015, which asks for a grant that is found, not argued.

## §3 Microsoft's WebView2 loader on Windows (FR-009, FR-013, FR-019)

**Decision**: settled two ways, as the owner chose at planning:
- **As a System Library.** GPL-3.0 §1 leaves System Libraries out of the
  Corresponding Source. The audit records why the loader is one: it is
  "included in the normal form of packaging a Major Component" (the WebView2
  SDK, the only way to use the WebView2 runtime that Windows includes). It
  "serves only to enable use of the work with that Major Component", since all
  it does is find and start the installed runtime.
- **And by a §7 additional permission** in HoploDex's license (§15), so the
  Windows build doesn't rest on that reading alone.
- **In the notices**: the loader is a hand-recorded Windows component, carried
  by `webview2-com-sys` (§7). Its notice is the WebView2 SDK's `LICENSE.txt`
  (BSD-3-Clause, Microsoft Corporation) from the `Microsoft.Web.WebView2`
  package at the version whose binaries `webview2-com-sys` 0.38.2 carries.
  The implementation finds that version in the `webview2-rs` repository and
  commits the text with its source.

_Amended 2026-10-18 (010 T009, T049): the SDK is `Microsoft.Web.WebView2`
1.0.3650.58 (BSD-3-Clause, "Copyright (C) Microsoft Corporation. All rights
reserved."). The static loader in `webview2-com-sys` 0.38.2 matches the
nupkg's by SHA-256. Its `LICENSE.txt` is committed at
`licenses/texts/webview2-loader/LICENSE.txt`, with the nupkg's address as its
`Source:` line._

**Rationale**: the license itself is compatible (BSD-3-Clause; the Eclipse
Foundation's IP log records the package as BSD-3-Clause). The open question is
GPL-3.0's requirement to provide source, which can't be met for a binary
Microsoft publishes without source. The copyright holder can remove the
question with a §7 permission. The FSF's FAQ gives the form, for a program
that links a library under a GPL-incompatible license or without source. Doing
both costs one paragraph.

**Alternatives considered**:
- The System Library reading alone. Possible, but it rests on an
  interpretation that a later reviewer might not share.
- Linking `WebView2Loader.dll` instead (the non-MSVC path). Rejected: it is
  the same library, now shipped beside the program, so it changes nothing.
- A loader with source. Rejected: none exists that `wry` uses. Writing one is
  out of scope.

## §4 Published Linux bundles and LGPL libraries (FR-017)

**Decision**: as the spec's clarification records:
- A release publishes the Flatpak (#42), whose WebKitGTK and GTK come from its
  Flathub runtime, which Flathub distributes with its source, and `.deb` and
  `.rpm`, which depend on the system's libraries.
- An AppImage can still be built (`scripts/build-appimage.sh`,
  `npm run bundle:linux`), but isn't published.
- The script's header and DEVELOPMENT.md's license audit section say so. The
  close-out lists the rule for #19's written release process: publishing an
  AppImage first needs a decision on how it offers its LGPL libraries' source.

Nothing in the build changes: `tauri.conf.json`'s `bundle.targets` is `"all"`.
Which bundles are uploaded is #19's.

## §5 Producing the backend's notices: our own script (FR-008, FR-010)

**Decision**: a Node script over Cargo's own tools, chosen at planning by the
owner over `cargo-about`. It lives beside `scripts/check-npm-licenses.mjs`, in
a new `scripts/licenses/` folder that both the audit and the notices use.

- **The shipped set**, per target triple:
  `cargo tree --locked --manifest-path src-tauri/Cargo.toml -e normal,no-proc-macro --target <triple> --features custom-protocol --prefix none -f '{p}'`.
  This uses Cargo's real feature resolver. Without `-e build` it leaves out
  build scripts' dependencies, and with `no-proc-macro` the procedural macros.
  The `hoplodex` package and any other path package are left out. A path
  package other than `hoplodex` fails the check, since nothing should ship
  from outside the registry. `cargo metadata` isn't used for the set: its
  resolve merges features across normal, build and development edges, and
  lists 37 crates on Linux that the binary doesn't contain (`html5ever`,
  `selectors`, `schemars`, `tray-icon` and others).
- **The target**: `TAURI_ENV_TARGET_TRIPLE` when Tauri runs the build. Tauri
  sets it for `beforeBuildCommand`, so `tauri build --target …` is followed.
  Otherwise the host's triple (`rustc -vV`). The `e2e` build uses the same
  `dist/`, and its notices are the release's: the WebDriver plugin is
  test-only and isn't listed (spec Assumptions).
- **Each crate's folder**: `cargo metadata --locked --format-version 1`'s
  `manifest_path`, in the registry source. `npm run build` runs before
  `cargo build` (Tauri's `beforeBuildCommand`), so on a fresh checkout the
  crates' sources aren't there yet. The script runs
  `cargo fetch --locked --target <triple>` first. That fetch is the one
  `cargo build` would make a moment later, from the same lockfile, and it does
  nothing once the sources are there.
- **Its license**: the `license` field, an SPDX expression. A choice (`OR`) is
  resolved to the first allowed license in a preference order that the audit
  and the notices share (MIT, Apache-2.0, BSD-3-Clause, BSD-2-Clause, ISC,
  Zlib, Unicode-3.0, BSL-1.0, 0BSD, MIT-0, CC0-1.0, MPL-2.0, Apache-2.0 WITH
  LLVM-exception). A conjunction (`AND`) keeps every part. The notice names
  the chosen license (spec Edge Cases, "a choice of licenses").
- **Its text**: the license files in the crate's root and in a `LICENSES/` or
  `license/` folder:
  - **Files that count**: `LICENSE*`, `LICENCE*`, `COPYING*`, `UNLICENSE*`,
    `NOTICE*`, `COPYRIGHT*`, `AUTHORS*` (the last three only beside a license
    file).
  - **A crate with one license** gets all of them.
  - **A crate with a choice** gets the files for the chosen license. A file is
    matched first by name (`LICENSE-MIT`, `LICENSE-APACHE`, `LICENSE.Apache-2.0`,
    `LICENSE-ZLIB`, and the like), then by content: the license's own opening
    phrase (MIT's "Permission is hereby granted, free of charge", Apache's
    "Apache License" and "Version 2.0", BSD's "Redistribution and use in source
    and binary forms", Zlib's "This software is provided 'as-is'", Unicode's
    "UNICODE LICENSE"), and a file that holds two licenses (`chrono`) counts
    for both.
  - **NOTICE files are always kept**: Apache-2.0 §4(d) asks for them, though
    none of today's crates has one.
  - **A file that only points elsewhere** (`siphasher`'s 281-byte `COPYING`,
    which names its licenses) isn't a text.
- **Fail closed** (FR-010):
  - **A crate with no text for its chosen license fails**, naming the crate,
    unless an exception (§8) gives its text.
  - **A file the matcher can't place fails** in a crate with a choice. A
    clarification entry names the files to use instead.
  - **There is no fallback to a generic license text.** That would drop the
    copyright lines, which are what the licenses ask to keep.
- **Size**: one license per crate, with identical texts stored once, comes to
  about 240 KB for Linux. The interface packages' texts add about 40 KB (§8).

_Amended 2026-10-18 (010 T016): the commands above don't work as written on the real tree. `cargo fetch` and `cargo metadata` also take `--manifest-path src-tauri/Cargo.toml` (they fail outside `src-tauri/`). `cargo tree -e build` alone follows only the app's own build edges and prints nothing for `cc`, so the build dependencies come from `cargo tree -e normal,build --depth 1 --invert cc --invert cmake`, and where cargo refuses a tool the graph doesn't have (no crate uses `cmake`) the script asks for each tool alone and skips the one that isn't there. A crate's enabled features come from the same per-target `cargo tree -f '{p} {f}'` call as the shipped set, not from `cargo metadata`'s resolve, which unions the features of every target and of development dependencies._

**Rationale**: no new tool on any machine that builds HoploDex. Every
`npm run build` produces the notices (FR-008), so `cargo-about` would have
been needed on the Linux container, both Windows setup scripts, the macOS VM
and `audit.yml`, and installing it needs the network. `cargo tree` gives the
shipped set exactly. The failure rules are this project's (FR-010): no silent
generic text. The audit already uses this approach for npm, so the two share
the allow list, the expression evaluator and the preference order.

**Alternatives considered**:
- **`cargo-about` 0.9.2** (#18's proposal; MIT OR Apache-2.0). Rejected by the
  owner at planning, for these reasons:
  - It must be installed everywhere a build runs.
  - When a crate has no license file, it falls back to the generic SPDX text,
    logged only at debug level, which loses the copyright lines.
  - Its graph comes from `cargo metadata` through `krates`, and wasn't
    confirmed free of the over-count above.
  - So its JSON would have needed a post-check against `cargo tree` anyway.
- **A generic text for crates with no license file**, taken from an SPDX
  license list. Rejected: it loses the copyright lines (FR-004, "the
  copyright and license text that license requires"), and no offline copy of
  the texts is in the toolchain anyway.

## §6 Producing the interface's notices (FR-008, FR-012)

**Decision**: the interface packages listed are the audit's shipped set,
computed by one function that the audit (`check-npm-licenses.mjs`) and the
notices share. The function changes in one way: a package marked
`devOptional` in the lockfile is development-only too.
- **What changes**: `@types/react`, `@types/react-dom`, `@types/prop-types`
  and `csstype` stop counting as shipped. They are only optional peers of
  shipped packages, installed for type checking, and are never bundled.
- **The build checks the set** (FR-012). A Vite plugin (§8) collects the
  `node_modules` package of every module in the main bundle and in the
  strength worker's. It fails the build, naming the package, when one of them
  isn't in the audit's shipped set. So the audit always covers what ships.
- **The other way round** is allowed: a shipped-set package that the bundle
  drops as unused code (today `loose-envify`, `js-tokens`, `detect-node-es`,
  `@radix-ui/react-arrow`, `@radix-ui/rect`, `@radix-ui/react-use-rect`) stays
  listed. It is a runtime dependency of what ships, and the bundler's choice to
  drop it can change with a minor update. Listing it costs a few lines. Missing
  one that ships would break its license.
- **Each package's text** comes from its folder in `node_modules`, with the same
  file rules as §5. For a package with a choice of licenses (`@tauri-apps/api`
  and `@tauri-apps/plugin-dialog`, Apache-2.0 OR MIT), the text is the chosen
  license's.
- **Two packages need an exception today** (§8):
  - `react-remove-scroll-bar` 2.3.8 ships no license file.
  - `@tauri-apps/plugin-dialog` 2.7.2 ships only `LICENSE.spdx`, SPDX metadata
    with a copyright line but no license text.
- **The fonts** are the three `@fontsource-variable/*` packages, listed like any
  package. Their `LICENSE` holds the OFL-1.1 text and copyright line. For Big
  Shoulders Display, a clarification corrects the copyright line (§12).

**Rationale**: one definition of "shipped" keeps the audit and the notices
from drifting apart (FR-012, SC-001). Checking the bundle's real module list
catches a package the lockfile rule would miss, such as a dependency wrongly
marked development-only.

**Alternatives considered**:
- Listing exactly the bundled packages. Rejected: then the notices would be
  the bundler's output, not the audit's set. FR-012 asks for the same
  components the audit counts, and SC-001 needs every component the audit
  counts to appear.
- Leaving `devOptional` counted as shipped. Rejected: it puts four
  development-only packages in the notices, against FR-008 and SC-001's "0
  development-only".

## §7 Components the tools can't see (FR-009, FR-016)

**Decision**: `licenses/hand-recorded.json` records each component the
dependency tools can't produce. It gives the component's name, version,
license, kind, the operating systems it ships on, its notice text (a file in
the carrying package, or one committed under `licenses/texts/`), the carrying
package (npm or crate), the carrier's version, and why it is recorded by hand.

Today's entries:

| Entry | Carrier | OS | Text from |
|---|---|---|---|
| SQLCipher 4.14.0 community (BSD-3-Clause) | crate `libsqlite3-sys` 0.38.1, feature `bundled-sqlcipher` | all | the crate's `sqlcipher/LICENSE` |
| SQLite 3.51.3 (public domain) | crate `libsqlite3-sys` 0.38.1 | all | the blessing, committed (`licenses/texts/sqlite/blessing.txt`) |
| OpenSSL (Apache-2.0), version read at build | crate `libsqlite3-sys` 0.38.1 | Windows | OpenSSL 3's `LICENSE.txt`, committed |
| Microsoft WebView2 loader (BSD-3-Clause) | crate `webview2-com-sys` 0.38.2 | Windows | the WebView2 SDK's `LICENSE.txt`, committed (§3) |
| OpenSubtitles 2024 word frequencies (ODC-BY) | npm `@zxcvbn-ts/language-en` 4.1.1 | all | the package's `NOTICE.md` and `THIRD_PARTY_LICENSES.md` |
| Moby Word Lists names (public domain) and random-name (MIT) | npm `@zxcvbn-ts/language-en` 4.1.1 | all | committed: the credit, and random-name's `LICENSE` from its repository |
| 1990 US Census surnames (public domain) | npm `@zxcvbn-ts/language-en` 4.1.1 | all | committed: the credit |
| Ten million passwords, Mark Burnett (public domain) | npm `@zxcvbn-ts/language-common` 4.1.3 | all | committed: the acknowledgement |
| EFF Large Wordlist for Passphrases (CC BY 4.0) | npm `@zxcvbn-ts/language-common` 4.1.3 | all | committed: §2's attribution |

_Amended 2026-10-18 (010 T021): the nine entries are as listed, with these
settled details. OpenSSL's text is `LICENSE.txt` at `openssl-3.6.5`, the version
vcpkg's port installs today (the Apache-2.0 text is the same across 3.x), and
the entry's carrier needs `libsqlite3-sys`'s `bundled-sqlcipher` feature like
SQLCipher's. The WebView2 loader's version is the SDK's, 1.0.3650.58 (§3). The
SQLite text is the blessing with a line saying where it is in the source. The
Moby entry carries its credit and random-name's MIT `LICENSE` (the repository at
468ae50d). Each data entry's `version` names its list's release (2024, 2001-01,
1990, 2015-02-09, 2016-07). Every committed text's first line is `Source: <url>`
and nothing between it and the text._

**Their licenses** are allowed for these entries only, as exceptions scoped
to them in `policy.mjs` (constitution, "Licensing": a license acceptable only
for particular packages is recorded as an exception):
- `blessing` (SQLite's SPDX identifier) for SQLite;
- `CC-BY-4.0` for the EFF list;
- `ODC-By-1.0` for the OpenSubtitles data, the existing exception;
- `LicenseRef-public-domain` for the Moby, Census and Burnett data.

None of them is allowed for a package or crate in general.

**The check** (FR-009) fails, naming the entry, when:
- the carrier isn't in the shipped set for an OS the entry names;
- the carrier's version differs from the recorded one;
- for an entry with a carrier feature, the feature isn't enabled;
- the entry's text file is missing.

**The OpenSSL version** comes from the build. On Windows the notices read
`OPENSSL_VERSION_STR` (or, for 1.1.x, `OPENSSL_VERSION_TEXT`) from the
`openssl/opensslv.h` that SQLCipher is compiled against, and they find that
header the way `libsqlite3-sys` 0.38.1's `build.rs` does (its `env()`, lines
312-321, and the include path, lines 159-189; confirmed against the source in
the Cargo registry, 2026-10-18; a new `libsqlite3-sys` version needs the
reading checked again, which the carrier-version check prompts):
- each variable is read as `<TARGET>_<name>` first (the triple upper-cased,
  dashes to underscores: `X86_64_PC_WINDOWS_MSVC_OPENSSL_DIR`), then as
  `<name>`, and a variable that is set counts even when it is empty;
- when `OPENSSL_LIB_DIR` and `OPENSSL_INCLUDE_DIR` are both set, the headers
  are in `OPENSSL_INCLUDE_DIR`;
- otherwise `OPENSSL_DIR` is required (the build panics without it on
  Windows), and the headers are in `OPENSSL_INCLUDE_DIR` if that is set, else
  in `<OPENSSL_DIR>/include`.

The failure names the variable that decided it, so a 3.x `OPENSSL_DIR` with a
1.1.1w `OPENSSL_INCLUDE_DIR` override is refused as 1.1.1w, naming
`OPENSSL_INCLUDE_DIR`. A Windows check where none of these gives the build its
headers fails, naming the variables. Only the headers are read: a library
directory that holds a different OpenSSL than its headers isn't seen. They
refuse a major version below 3, because 1.1.x's license isn't compatible (§1),
so that rule is now a build check, not only a note. The check with
`--all-targets` (§9) runs on Linux, where the Windows build's variables don't
exist, so it can't read a version. There the entry reads "3.x (the version the
Windows build links)", and the check confirms only the text and the carrier.

**A guard for new native code**: `licenses/native-code.json` lists every
shipped crate that compiles code with `cc` or `cmake` (an enabled build
dependency in `cargo tree -e build` for the target) or carries a prebuilt
library (`*.lib`, `*.a`, `*.dll`, `*.dylib`, `*.so`, `*.o` in its package).
Each entry says what that code is:
- a hand-recorded entry;
- "the crate's own code";
- "import libraries for system DLLs";
- "links a system library".

A crate that turns up and isn't listed fails the check, naming it. That makes
today's manual check, "C code built inside a crate" (DEVELOPMENT.md), an
automated one. It is how §3's loader would have been found. The initial list:
- `libsqlite3-sys` (hand-recorded);
- `webview2-com-sys` (hand-recorded);
- `objc2-exception-helper` (its own code);
- the `windows_*_msvc`/`windows_*_gnu` crates (import libraries);
- the `-sys` crates that only link system libraries, such as GTK, WebKitGTK,
  `libdbus` and Core Foundation. These are found by `links` in the manifest,
  and listed only if they also carry files or build C.

_Amended 2026-10-18 (010 T021): on the three real trees only four crates need
an entry: `libsqlite3-sys`, `objc2-exception-helper`, `webview2-com-sys` (it
carries the loader for three architectures, `WebView2Loader.dll` and the
static `.lib`) and `windows_x86_64_msvc` (two versions, one entry; its import
libraries). No `-sys` crate builds C or carries a prebuilt file, and no
`windows_*_gnu` crate ships on these targets, so none is listed (a listed
crate no target ships fails `--all-targets`)._

**Rationale**: FR-009 asks for "recorded once" with a carrier check, and the
loader showed the tools miss more than the spec named. A record kept by hand
can only stay true if something fails when what it describes changes.

**Alternatives considered**: hand-recorded entries with no carrier version.
Rejected by FR-009.

## §8 Exceptions and clarifications (FR-010)

**Decision**: two more files beside `hand-recorded.json`:
- **`licenses/exceptions.json`**: a shipped component whose license text
  isn't in its package.
  - **What an entry gives**: the component, the version it was checked at,
    the text file (committed under `licenses/texts/<component>/`), where the
    text was taken from (the source repository's file at that version's tag,
    with its URL), and why.
  - **When the check fails**: the component's declared license changes, or it
    starts shipping a license file of its own. The exception is then stale, and
    the failure says to remove it.
  - **A version bump alone doesn't fail.** The text is the project's license
    file, which rarely changes between versions, and failing every Dependabot
    bump of `objc2` would train people to update entries without looking. The
    recorded version shows how old the check is, and the release review's
    manual list (FR-018) includes rechecking exceptions whose version is
    behind.
  - **Today's entries**: on Linux, `alloc-stdlib`, `dlopen2`, `siphasher`,
    the five `unic-*` crates, `zune-core` 0.4 and `zune-jpeg` 0.4. On macOS,
    `block2`, `dispatch2`, `objc2`, `objc2-app-kit`, `objc2-core-foundation`,
    `objc2-encode`, `objc2-exception-helper`, `objc2-foundation`,
    `objc2-web-kit`. On Windows, `webview2-com` and `webview2-com-sys`. On
    npm, `react-remove-scroll-bar` and `@tauri-apps/plugin-dialog`. That is 21
    crates and 2 packages. Where several crates share one repository and one
    license file (the `objc2` and `unic` families, the two `zune` crates, the
    two `webview2` crates), they share a text file.
  - _Amended 2026-10-18 (010 T021): the real trees differ from the list
    planned: `siphasher` (its `COPYING` only names its licenses) and
    `objc2-foundation` were missing from it. Three texts come from a commit
    later than the checked version, because the repository had no text then:
    the `objc2` family's `LICENSE-MIT.txt` (ee9a7ada, "Include license text in
    the repository"), `siphasher`'s `LICENSE-MIT` (07a910ae) and
    `react-remove-scroll-bar`'s `LICENSE` (the repository has no tag for
    2.3.8). The `zune` crates' repository holds no MIT or Apache-2.0 text at
    all, only Zlib's and a statement of the three-way choice, so both are
    committed and the notice is under Zlib: an exception for a choice is used
    under the first allowed alternative (in the preference order) that one of
    its committed texts is a text of, placed by name and then by content as a
    package's files are (a text under 400 bytes that no name places only
    points, so the repository's statement of the choice doesn't count), and it
    fails, naming the component, when none places. The
    other crates take the text at the commit their package records
    (`.cargo_vcs_info.json`)._
  - _Amended 2026-10-18 (010 T021): owner decision. The `objc2` crates are
    derived from Apple SDKs shipped with Xcode, and their repository's
    [LICENSE.md](https://github.com/madsmtm/objc2/blob/8852b424193ca41602281b3d7540d7c8ed51e49a/LICENSE.md)
    says "it is unclear whether distributing derived works such as these crates
    are allowed". That is accepted: the crates are used under their stated
    licenses, as the objc2 project reasons (using them needs Xcode, whose
    license the builder has agreed to). The notices don't change._
  - **A choice whose preferred license has no text** (`brotli-decompressor`:
    `BSD-3-Clause/MIT`, ships BSD-3-Clause's text; `dunce`: `CC0-1.0 OR MIT-0
    OR Apache-2.0`, ships CC0's) is not an exception. The notice names the
    first allowed alternative the package has a text for (`findTextsAmong`,
    in `licenseFiles.mjs`), and fails only when none has (§5, "Fail
    closed"). An older crate writes a choice with a slash (`MIT/Apache-2.0`);
    it is read as `OR`, as cargo and cargo-deny read it.
- **`licenses/clarify.json`**: for a crate whose files the matcher places
  wrongly or can't place, it names the files to use, with the SHA-256 of each.
  If a file changes, the clarification fails and is reviewed. This is the Big
  Shoulders font correction's home too (§12).

**Rationale**: FR-010 allows exceptions "that record where the text comes from
and why". Committing the text, not fetching it, keeps the build offline. The
owner is wary of builds that reach the network (memory of 007's planning).

## §9 Where the checks run (FR-008, FR-010, FR-012)

**Decision**:
- **Every build**: `npm run build` runs the Vite plugin, which produces the
  notices for the build's target (§5) and fails the build on any failure in
  §5 to §8. So `tauri build`, `tauri dev`'s first build and the E2E build all
  carry notices that match.
- **The audit**: `npm run audit:licenses` gains
  `node scripts/licenses/notices.mjs --check --all-targets`, which runs §5 to
  §8 for Linux, Windows and macOS from any machine. `cargo tree --target`
  needs no other toolchain, and `cargo deny` already fetches every target's
  crates. The Dependency audit workflow (`audit.yml`) runs on Linux, so it now
  catches a macOS or Windows notice failure on every pull request, before
  anyone builds on that OS.
- **The allow list is in step**: a test compares the script's `ALLOWED` with
  `deny.toml`'s `allow` list. DEVELOPMENT.md asks people to keep the two in
  step, and nothing checked it.

## §10 What the app reads, and when (FR-003, FR-004, FR-006, SC-004, SC-005)

**Decision**:
- **What the build writes**:
  - `dist/notices.json`: the program block (version, copyright years, source
    address), the GPL-3.0 text from `LICENSE`, the additional permission (§15),
    the target, every component, and their texts, each stored once
    (contracts/notices.md). About HoploDex fetches it when it opens.
  - `dist/NOTICES.txt`: the same content as plain text, for the bundles (FR-011;
    #19 copies it next to the program).
- **What the dialog shows before the fetch**: the program block it needs
  first (name, version, copyright years, source address) is compiled into the
  bundle through Vite `define` constants. The legal statement therefore shows
  at once, even if the fetch were slow (SC-003, SC-004).
- **The fetch is the app's own file**. It goes through the same protocol that
  serves the page (`tauri://localhost`, or `http://tauri.localhost` on
  Windows), within the CSP's `connect-src 'self'`.
  - **No command**: no IPC command and no database access are involved
    (FR-001, SC-005). No new capability is needed.
  - **No network**: the CSP already blocks any other origin
    (`photos-documents.e2e.ts`'s check), and the app has no opener permission.
- **The size and its cost**: about 300 KB for Linux, and it loads once per
  opening. Parsing it takes milliseconds. The list renders collapsed rows only
  (about 340 names); a notice's text renders when its row opens, so long texts
  never all render at once.
- **Development**: under `vite` (the development server), the plugin serves
  `/notices.json` computed on request. Development builds aren't held to §9's
  checks (spec Edge Cases), so the plugin reports a failure there as a warning
  and serves what it could find.

**Alternatives considered**:
- A backend command that returns `include_str!`'d notices. Rejected: it adds a
  command, an ACL entry and a capability for data that is already a static
  asset of the page, and the notices aren't the backend's to own.
- Importing the JSON into the bundle. Rejected: the bundled-package check runs
  after bundling, so the content isn't known when the bundle is made. It would
  also add about 300 KB to the start-up script for a screen rarely opened.

## §11 About HoploDex: one screen, three ways in (FR-001, FR-005, FR-007)

**Decision**:
- **One dialog, one owner**. `AboutDialog` is rendered once, by
  `SessionProvider`, which is above both the chooser and the open database's
  tree. The open/closed state lives there too.
- **Three ways in**, all calling `session.showAbout()`:
  - the chooser's top bar gains an "About HoploDex" button beside the theme
    toggle;
  - the database menu gains "About HoploDex", after "About databases and
    security";
  - on macOS, the application menu's About item emits `app:about-requested`,
    which `SessionProvider` listens for, as it does for `app:quit-requested`.
- **It closes when the session changes**: any change of phase (an open, a
  lock, a close, a switch) closes it. So the spec's edge case, "a database
  locks or closes while About HoploDex is open from its menu", behaves as
  every screen of an open database does, and About from the chooser closes
  when a database opens.
  _Amended 2026-10-18 (010 T053): the move from the starting phase to the
  chooser is no change of session, so About opened from the menu bar while the
  window starts stays open. Every other phase change closes it
  (contracts/ui.md §1)._
- **The macOS menu**: `main.rs`'s `setup()` builds Tauri's default menu
  (`Menu::default`) and replaces the application submenu's predefined About
  item with a `MenuItem` of id `about`, "About HoploDex". `on_menu_event`
  emits the event for that id. Every other item is untouched: Services, Hide,
  Quit (which `platform::ask_before_terminate` already handles), and the Edit
  and Window menus. Linux and Windows have no application menu and are
  unchanged.
- **Layout** (contracts/ui.md): the shared `Dialog`, size `lg`, "About
  HoploDex".
  - **The program block**: name, version, the copyright line, the license
    statement, the no-warranty statement, the additional permission, and the
    source address in a select-all monospace span, as export's paths are
    shown. No copy button: the app has no clipboard permission, and the
    selection's Copy works.
  - **"GNU General Public License, version 3"**: a `Disclosure` that shows the
    full text when opened.
  - **"Built with"**: a search field ("Find a component") and the list, one
    `Disclosure` per component, titled with its name and summarised as
    "version · license". Opened, it shows the notice texts in a scrolling
    monospace block.
  - **No match**: "No component matches “…”.".
- **Reachability** (SC-003):
  - The license statement: chooser, 1 action. Database menu, 2 (the menu, the
    item).
  - The full GPL: one more, so 2 and 3.
- **Filtering** runs on every keystroke, with no debounce. Matching a name
  against about 340 names takes well under a frame.

**Alternatives considered**:
- Two pages in the dialog (About, Licenses). Rejected: one more action to
  reach the components, and a second navigation pattern.
- A list beside a detail pane. Rejected: it doesn't fit a narrow window
  (FR-007), and the disclosure pattern already exists.
- macOS's own About panel filled with Tauri's `AboutMetadata`. Rejected by
  the spec's clarification.

## §12 The Big Shoulders copyright line

**Decision**: the notice for `@fontsource-variable/big-shoulders-display`
carries the copyright notice that the font files themselves carry (the
`name` table's copyright string, ID 0, in the shipped `woff2`) and the
upstream project's `OFL.txt` ("The Big Shoulders Project Authors", at the
upstream revision the font was built from). A `clarify.json` entry does it,
with the text committed and its source recorded. The package's `LICENSE`
says "Google Inc.", which looks like a mistake in Fontsource's generator.
OFL-1.1 asks that the copyright notice be kept, and the font's own notice is
the authoritative one. The implementation reads ID 0 from each of the seven
shipped `woff2` files. If the Atkinson files disagree with their packages'
`LICENSE` too, they get the same treatment.

_Amended 2026-10-18 (010 T010):_ the seven files, read by a script using only
`node:zlib`'s Brotli (a build's `dist/assets/` copies are byte-identical to the
packages' `files/*-wght-normal.woff2`, 5.3.0). Every file has the one `name`
ID 0 record (platform 3, English US):

| Package | Files (latin, latin-ext, vietnamese where shipped) | Copyright string (ID 0) |
|---|---|---|
| `@fontsource-variable/atkinson-hyperlegible-mono` | latin, latin-ext | `Copyright 2020-2024 The Atkinson Hyperlegible Mono Project Authors (https://github.com/googlefonts/atkinson-hyperlegible-next-mono)` |
| `@fontsource-variable/atkinson-hyperlegible-next` | latin, latin-ext | `Copyright 2020-2024 The Atkinson Hyperlegible Next Project Authors (https://github.com/googlefonts/atkinson-hyperlegible-next)` |
| `@fontsource-variable/big-shoulders-display` | latin, latin-ext, vietnamese | `Copyright 2019 The Big Shoulders Project Authors (https://github.com/xotypeco/big_shoulders)` |

The latin and latin-ext files of a family carry the same string. Neither
Atkinson family disagrees with its package's `LICENSE`: that file's first line
is the same string (it also lists the italic file's name before a second copy
of it, which the notice need not repeat), so the Atkinson notices need no
`clarify.json` entry. Only Big Shoulders does: its package `LICENSE` says
"Google Inc.".

## §13 Version, copyright years and source address (FR-002)

**Decision**:
- **The version**: `package.json`'s `version`, compiled in with `define`.
  `tests/app_version_test.rs` already keeps it equal to `APP_VERSION`, so the
  version shown is the running build's.
- **The years**: "2026" to the year of the build's commit (`git log -1
  --format=%cs`), so a rebuild of a tag gives the same text. With no git, the
  build's own year. The release's commit is the release, so "the year of the
  build's release" holds. "2026" alone when that year is 2026.
- **The source address**: `https://github.com/exodious/HoploDex`, from
  `package.json`'s `repository.url`, without `git+` and `.git`. It is shown,
  not opened (FR-006).

## §14 Testing (constitution II)

**Decision**:
- **The generator's rules** (§5 to §8): Vitest tests in `scripts/licenses/`,
  against fixtures under `scripts/licenses/fixtures/` (a small fake registry
  of crate folders, a fake `node_modules`, a fake `cargo tree` output). Each
  failure rule has a red fixture: a missing license file, a file the matcher
  can't place, a stale exception, a carrier version change, a missing carrier
  feature, an unlisted native-code crate, a bundled package outside the
  audit's set. Each FR-010 or FR-009 message names the component.
- **Against the real trees**: `--check --all-targets` in `npm run
  audit:licenses` (§9). A Vitest test also runs the check for the host's
  target and compares its component list with the audit's own counts
  (FR-012, SC-001), and checks that target's real `NOTICES.txt` (FR-011).
  Each OS's test run therefore checks its own notices. It is skipped, with a
  message, where `cargo` isn't on the PATH.
- **The app**:
  - Vitest for `AboutDialog`: the statements, the disclosure, filtering and no
    match, a failed fetch, the entry points calling `showAbout`, and closing on
    a phase change.
  - One E2E journey in `databases.e2e.ts`, whose setup is the chooser and
    creating and opening a database: from the chooser and from the database
    menu, reading the license, finding a component, this OS's components
    listed and the count equal to the build's `notices.json`, scrolling the
    whole list without a long frame, and the database file's and
    `machine.json`'s SHA-256 unchanged across About.
  - The macOS menu item through `chooseMenuBarItem`, which the ⌘Q test
    already uses.
  - The screenshot walk, in both themes.

## §15 The additional permission (FR-019)

**Decision**: the text, adapted from the FSF's form for a GPL-incompatible
library:

> Additional permission under GNU GPL version 3 section 7
>
> If you modify this Program, or any covered work, by linking or combining it
> with Microsoft's WebView2 loader (WebView2Loader or WebView2LoaderStatic,
> from the Microsoft WebView2 SDK), or a modified version of that library,
> containing parts covered by the terms of the Microsoft WebView2 SDK's
> license, the licensors of this Program grant you additional permission to
> convey the resulting work.

The FSF form's optional last sentence, that Corresponding Source includes the
library's source, is left out, since none is published.

- **Where it lives**:
  - `licenses/additional-permission.txt`, the one copy that the notices read;
  - About HoploDex, after the license statement;
  - `NOTICES.txt`;
  - a new "License" section in `README.md`, which holds the copyright notice,
    the GPL notice and this permission (the "repository's license notice"
    FR-019 names).
- **Kept in step**: a test fails if the README's copy differs from the file.
- **`LICENSE` stays the GPL's text, unmodified**: GPL-3.0 forbids changing it,
  and the FSF puts additional permissions beside it.
- **The SPDX identifier stays `GPL-3.0-only`** in `package.json` and
  `Cargo.toml`. SPDX has no identifier for this permission, which only grants
  more. A `LicenseRef` would make every license tool treat the program as
  unknown.
- **Contributions**: the README's section says that contributions are taken
  under the license with this permission. So a later contributor's code
  carries it too, and the copyright notice's "and the HoploDex contributors"
  (FR-002) holds.

## §16 Security and data handling (for the PR description)

- **Nothing new crosses the IPC boundary**: no new command, no new
  capability, and one new event, which carries no data.
- **No database, setting or passphrase is read** by About HoploDex (FR-001,
  SC-005). It reads a static asset of the page.
- **No network**: the CSP's `connect-src 'self'` and `default-src 'self'`
  stand, there is no opener permission, and the source address is text.
- **The build reads only local files**: the lockfile, `node_modules`, Cargo's
  registry source and the committed `licenses/` folder. Its one possible
  download is `cargo fetch --locked` (§5), the same crates from the same
  lockfile that `cargo build` downloads anyway. No license text is fetched:
  exceptions are committed (§8).
- **The strength hint loses one list**, so it guesses lower for some
  Wikipedia words. It is advice (003 FR-003); the passphrase rules are
  unchanged.
