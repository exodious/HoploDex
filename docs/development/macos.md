# macOS

_Part of the [development docs](../../DEVELOPMENT.md)._

## What runs on macOS

**macOS (partly ported, #28):** the harness gives the app a `HOME` inside
the sandbox (only the app; cargo keeps the real one), since macOS builds
Application Support, Caches and Documents from `HOME` and ignores `XDG_*`.
It runs on the host, with the app's window on the real desktop. It needs an
unlocked, logged-in session: while the screen is locked, WKWebView reports
every page as hidden and gives it no animation frames, so every step times
out with "The app never settled". The tests that use real input
(`e2e/support/realInput.ts`, X11 only: `databases`' owl beak, `backups`' restore-dialog
regressions, one test each in `browse-search`, `browser-controls`,
`document-preview` and `locking`, all of `cartridges-actions`, `regulated-items` and the
`accessories-*` files) are skipped, and reported as
pending. `quit-cleanup.py` runs there too (see [Quit cleanup](e2e.md#quit-cleanup)), and so does the
screenshot walk, but macOS keeps a window within the screen, so the
full-page shots are cut off at the screen's height unless the display is
taller than the page (`scripts/tart-vm.sh start --tall` gives its VM
1920×4200, as Linux's Xvfb screen).

## macOS in a VM

**macOS in a VM:** to test macOS without your own account's data nearby,
use a [tart](https://tart.run) VM made from a Cirrus Labs image with Xcode
(`ghcr.io/cirruslabs/macos-tahoe-xcode`). `scripts/tart-vm.sh` sets one up
and runs the tests in it as a standard (non-admin) user:

```bash
scripts/tart-vm.sh setup                   # clone, create hoplotest, install the toolchain, log it in at boot
scripts/tart-vm.sh start                   # start it with a 1920×1080 screen, for the tests and manual testing
scripts/tart-vm.sh test                    # copy the checkout in, lint and test over SSH, E2E in its desktop
scripts/tart-vm.sh ssh npx vitest run src/features/firearms/FirearmForm.fields.test.tsx   # one command
scripts/tart-vm.sh gui npm run test:e2e -- --spec e2e/specs/record-firearm.e2e.ts
scripts/tart-vm.sh start --tall            # restart it with a 1920×4200 screen, for the screenshot walk
scripts/tart-vm.sh gui bash -c 'npm run build && npm run screenshots'
scripts/tart-vm.sh fetch e2e/screenshots-out   # copy results back
scripts/tart-vm.sh start                   # back to 1920×1080
```

The VM's screen is 1920×1080 unless you ask for the tall one. The screenshot
walk needs 1920×4200, since macOS keeps a window within the screen and the
full-page shots grow to 4000, but tart's window shrinks a screen that tall
until the desktop is too small to use by hand. tart changes the screen only
while the VM is stopped, so `start` shuts a running VM down and starts it
again when it has the other size. It also turns off tart's display refit
(`--no-display-refit`), which otherwise reshapes the VM's screen to fit tart's
window (one that remembered an earlier size gave a 1920×2012 screen for
1080), so the screen is the size asked for and tart scales it into the
window. Set `HOPLODEX_VM_DISPLAY` (such as
`1440x900px`) for a usual screen that suits your own; keep it at least
1200×800 for the E2E window.

`setup` uses the image's `admin` only to create the user, turn off sleep,
skip the user's Setup Assistant, log it in automatically at boot and give
Terminal and SSH sessions Accessibility and Screen Recording, and turn off
the "bypass the private window picker" question macOS otherwise asks each of
them at the first screenshot and every 30 days (`forceBypassScreenCaptureAlert`,
as a managed preference that a small LaunchDaemon puts back after each boot).
The toolchain (rustup, cargo-nextest, Node and npm at the `Dockerfile`'s
versions) goes in the user's home. Running it again finishes what's left.
Shut the VM down with `scripts/tart-vm.sh stop`, not `tart stop`, which cuts a macOS guest's power
and loses whatever it hasn't written to disk yet. The script's header lists
its settings (`HOPLODEX_VM` and friends, for a VM you already have). The rest
of this paragraph is what it does for you. To set a VM up by hand, install the
prerequisites in it and leave it logged in at its desktop. Copy the checkout in with
`rsync -a --exclude .git --exclude src-tauri/target --exclude node_modules
--exclude dist --exclude e2e/screenshots-out ./ <user>@$(tart ip <vm>):HoploDex/`
(with `--delete`, exclude those by name even if `.gitignore` is a filter too:
macOS's rsync deletes files a dir-merge filter alone matches).
Over SSH, mind where a GUI app starts. A program started from an SSH session
runs in launchd's `Background` session (`launchctl managername` says so) and
never gets a window: the app starts, logs, and waits forever. Run the E2E
suite from a Terminal in the VM, or from SSH start the app through Terminal
(`open -a Terminal run.command`) and drive it from SSH, as
`scripts/macos/pdf-surface-check.sh` does. Whatever posts mouse input or
takes screenshots (`sshd-keygen-wrapper` over SSH, or Terminal) needs Accessibility
and Screen Recording in the VM's Privacy & Security settings. Until someone
answers it, the first request's prompt sits over the middle of the screen
and catches the clicks. The tart VM and podman's VM together can need more
memory than a host has; if so, run the dev container's checks on another
computer.

## Driving the Mac from Linux, over SSH

**Driving the Mac from Linux, over SSH.** Agents run on Linux and reach
macOS through the Mac host, the Mac that runs the tart VM, over SSH;
`scripts/tart-vm.sh` there does the rest. A non-interactive SSH session
doesn't read the login profile, so Homebrew's tart isn't on its `PATH`:
start every command with `export PATH=/opt/homebrew/bin:$PATH`.

```bash
mac() { ssh -n <user>@<mac-host> "export PATH=/opt/homebrew/bin:\$PATH; cd ~/hoplodex-<task> && $1"; }
ssh -n <user>@<mac-host> 'git clone -q -b <branch> <repository> ~/hoplodex-<task>'   # once per task, after pushing <branch>
mac 'git pull -q --ff-only'                     # after each push
mac 'scripts/tart-vm.sh start'
mac 'scripts/tart-vm.sh test'                   # or sync, then gui for one command:
mac 'scripts/tart-vm.sh sync && scripts/tart-vm.sh gui npm run test:e2e -- --spec e2e/specs/record-firearm.e2e.ts'
mac 'scripts/tart-vm.sh fetch e2e/screenshots-out'
mac 'scripts/tart-vm.sh stop'                   # always, even after a failure
scp -r '<user>@<mac-host>:hoplodex-<task>/e2e/screenshots-out' e2e/   # results back to Linux
```

- **The checkout** is a clone of its own for each task, in the host
  account's home, never someone's existing checkout, which may hold work
  in progress. Code goes over through GitHub: push the branch, then pull it
  there. `sync` (and `test`, which syncs first) copies it into the VM.
- **The order** is `start`, then `test` (or `sync` and `gui`), then `fetch`,
  then `stop`. Stop the VM even when a step fails, so the next run finds the
  host free.
- **One VM at a time** on the host, with at most 8 GB
  (`HOPLODEX_VM_MEMORY`, 8192 by default): if `tart list` shows one
  running, wait for it rather than starting another.
- **One GUI or E2E run at a time** in the VM, as on Windows.
