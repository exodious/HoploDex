# Licenses

_Part of the [architecture docs](../architecture.md)._

`scripts/licenses/` and `licenses/` (feature 010) make the third-party notices
and keep the license audit honest.

`shipped.mjs` is the one definition of what ships (`check-npm-deprecated.mjs`,
feature 012, uses its npm definition too; npm: the lockfile's packages that are
neither `dev` nor `devOptional`; crates: `cargo tree -e normal,no-proc-macro
--features custom-protocol --target <triple>`, `hoplodex` excluded) and
`policy.mjs` the allow list and choice order that `check-npm-licenses.mjs` and
`deny.toml` share (a test keeps `deny.toml` in step); `spdx.mjs`,
`licenseFiles.mjs` (which of a package's files is its license text) and
`records.mjs` (the four record files and their checks) feed `notices.mjs`, the
generator and CLI (`node scripts/licenses/notices.mjs --check [--all-targets |
--target <triple>]` or `--out <dir>`), and `vitePlugin.mjs` runs it in `vite
build` (`hoplodexNotices()`, in `vite.config.ts` for the main build and any web
worker's, of which there are none since the strength worker went (2026-11-04);
it also fails a bundled package outside the shipped set and withdrawn data in a
chunk, of which none is listed now).

Every check fails closed, naming the component; there is no generic-text
fallback, and a version bump fails a hand-recorded entry but not an exception.

`npm run build` writes `dist/notices.json` and `dist/NOTICES.txt` for the
build's target (`TAURI_ENV_TARGET_TRIPLE`, else the host's); `npm run
audit:licenses` also runs `--all-targets`.

`licenses/` holds the records: `hand-recorded.json` (what the tools can't see:
SQLCipher, SQLite, Windows' OpenSSL and WebView2 loader, and the `zxcvbn`
crate's word lists with their grounds, two of them as
`LicenseRef-word-frequencies`, 010 FR-015's exception), `exceptions.json` (a
shipped component whose package leaves out its license text, the text committed
under `licenses/texts/` with a `Source:` first line), `clarify.json`,
`native-code.json` (every shipped crate that builds C or carries a prebuilt
library), and `additional-permission.txt`, the GPL section 7 permission for the
WebView2 loader, whose one copy the notices, About and the README's License
section share.

"License notices" in `docs/development/licenses.md` says how to answer each
failure.
