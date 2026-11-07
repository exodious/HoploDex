# Flatpak

_Part of the [architecture docs](../architecture.md)._

`flatpak/` (feature 011) holds what builds the Flatpak: the manifest
(`io.github.exodious.HoploDex.yml`), `cargo-sources.json` and
`node-sources.json` (generated from the lockfiles by `npm run flatpak:sources`;
`scripts/flatpak/sources.test.ts` fails when either is stale), `flathub.json`,
`linter-exceptions.json`, the desktop entry, the metainfo and its screenshots
(the five store shots in `flatpak/screenshots/` are retaken only on purpose, by
`npm run screenshots:store` in the dev container started with `--store-shots` (a
tmpfs at `/home/alex/Documents` for the one command, so the chooser's path reads
like a user's), never by a build: `e2e/screenshots/store.e2e.ts` takes them from
the seed with `--fixed-times` so no text in view depends on the day,
`store-shots.json` records what each shows and
`scripts/flatpak/storeShots.test.ts` checks it against research.md §13; "The
Flatpak's store shots" in `docs/development/screenshots.md`).

`scripts/flatpak/` holds `guard.mjs` and `env.sh` (the host guard: the scripts
never use the developer's own Flatpak installation), `build.sh`, `check.sh`,
`contents.mjs` (the allow list of what the bundle may carry) and
`checkManifest.mjs`.

`npm run bundle:flatpak` and `npm run test:flatpak` (which runs
`e2e/specs/flatpak.e2e.ts`, excluded from every other run) work only in
`scripts/dev-container.sh --flatpak`, or on a GitHub-hosted runner that opted in
(`HOPLODEX_CI_FLATPAK=1`, with the workflow making the check's stand-ins);
"Flatpak" and "The Flatpak check" in `docs/development/flatpak.md` say how they
work.

The check's stand-ins: a tmpfs at `/run/media/dev/hoplodex-check` for a
removable drive (inside the reach), `/mnt/hoplodex-check` and
`/srv/hoplodex-check` as locations it shows refused,
`scripts/flatpak/openuri_mock.py` as the desktop portal (OpenURI and a Secret
portal handing back a fixed key; `src-tauri/examples/flatpak_keyring_read.rs`
reads the app's keyring file back with it), and a headless
`gnome-keyring-daemon` kept on the bus only to show `org.freedesktop.secrets`
refused.
