# Install and build

_Part of the [development docs](../../DEVELOPMENT.md)._

## Install dependencies

```bash
npm install
```

## Build

```bash
npm run build                                              # tsc typecheck + vite build -> dist/ (with notices.json and NOTICES.txt)
cargo build --manifest-path src-tauri/Cargo.toml            # backend, debug profile
npm run tauri dev                                           # run the full app in dev mode (hot reload)
npm run tauri build                                         # production installer/bundle for this OS
npm run bundle:linux                                        # the same on Linux, with a working AppImage (below)
```

The bundles are the ones `bundle.targets` in `src-tauri/tauri.conf.json`
names: the `.deb`, `.rpm` and AppImage on Linux, the NSIS installer on
Windows (no MSI), the `.app` and `.dmg` on macOS. What a release publishes is
in [RELEASING.md](../../RELEASING.md). A published package carries
`NOTICES.txt` beside the program, which `src-tauri/tauri.bundle.conf.json`
adds: `npm run bundle:linux` passes it, and elsewhere add
`--config src-tauri/tauri.bundle.conf.json` to `tauri build`, after
`npm run build` has written `dist/`. It isn't in `tauri.conf.json` because
`tauri-build` copies a bundle's files on every Cargo build, which would then
fail without `dist/`. On macOS, Tauri signs the `.app` ad-hoc, with the
hardened runtime on (`signingIdentity: "-"`, RELEASING.md's "Signing"); in the macOS VM, build the
`.dmg` with `CI=true`, which skips `bundle_dmg.sh`'s Finder layout step: it
needs Automation access that the VM's session doesn't have, and fails
("failed to run bundle_dmg.sh") without it. `node scripts/check-package.mjs deb|rpm|nsis|dmg <file>`
checks a built package.

`npm run build` also writes the third-party notices, `dist/notices.json` and
`dist/NOTICES.txt`, for the build's target, and fails when they can't be made
(see [License notices](licenses.md#license-notices)). Run it before `test:e2e` or
`screenshots` as usual: the E2E build embeds them with the rest of `dist/`.

Cargo never deletes old build outputs. Each change to a test or a dependency
leaves the previous test programs and incremental caches behind in
`src-tauri/target`, so it grows by gigabytes a week. Clear it every so often:

```bash
cargo clean --manifest-path src-tauri/Cargo.toml                                # on the host
scripts/dev-container.sh find src-tauri/target -mindepth 1 -delete             # the container's target volume
```

In the container, `src-tauri/target` is the volume's mount point. `cargo
clean` empties it, then fails because it can't remove the mount point
itself, so the container command empties the volume with `find` instead.

## Linux AppImage

On Linux, build the bundles with `npm run bundle:linux` (in the dev
container: `scripts/dev-container.sh npm run bundle:linux`), not
`npm run tauri build`. Arguments go through to `tauri build`, so
`npm run bundle:linux -- --bundles appimage` builds only the AppImage.

A plain `tauri build` AppImage carries the build machine's display-stack
libraries (`libwayland-*`, `libxkbcommon`, `libxcb-*`, `libXau`, `libXdmcp`)
and puts them ahead of the host's. On a host with a newer Mesa than the build
machine (Arch and CachyOS, Fedora 44), WebKitGTK then can't create an EGL
display: the window opens grey, and the terminal shows
`Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...`
([tauri-apps/tauri#15976](https://github.com/tauri-apps/tauri/issues/15976)).
`scripts/build-appimage.sh` sets `LINUXDEPLOY_EXCLUDED_LIBRARIES` so
linuxdeploy leaves those libraries out and the host's are used, then extracts
the AppImage and fails if any are still inside. Only the linuxdeploy that
Tauri CLI 2.12 and later downloads reads that variable. The `.deb` and `.rpm`
use the system's libraries anyway.

To check an AppImage on a machine with a newer Mesa without a desktop session
or your real data, run it under `xvfb-run` with throwaway `HOME` and `XDG_*`
directories and no session D-Bus. The broken build aborts at once with the EGL error, and the
fixed one shows the chooser.
