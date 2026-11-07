# Windows

_Part of the [development docs](../../DEVELOPMENT.md)._

x64 Windows 10 or 11 (Pro, Enterprise or Education), or Windows Server
2025 with Desktop Experience. Two scripts install everything in this
section, skipping what's already there, so they can be run again:

```powershell
# 1. Once per computer, from an elevated PowerShell; -AutoLogon (test machines) and -GitHubCli optional
powershell -ExecutionPolicy Bypass -File scripts\windows\setup-system.ps1 -User alice
# 2. Signed in as that user, from a new PowerShell that isn't elevated;
#    -ClaudeCode optional
powershell -ExecutionPolicy Bypass -File scripts\windows\setup-user.ps1
```

On a new computer without Git, copy the two scripts over first; the system
script installs Git, and `-GitHubCli` adds `gh`. Run the user script as the account that builds: rustup, npm 12 and
the cargo tools install into that account's profile, so an elevated shell
signed in as another administrator sets up the wrong account.

## What the scripts install

What they install, to set it up by hand instead:

**Machine-wide** (`setup-system.ps1`):

- **The MSVC C++ build tools and a Windows SDK.** Rust's
  `x86_64-pc-windows-msvc` toolchain links with Microsoft's linker, and
  `rusqlite`'s `bundled-sqlcipher` compiles SQLCipher's C with `cl.exe`.
  Install [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  2026 with the "Desktop development with C++" workload, whose recommended
  parts include the SDK:

  ```powershell
  winget install --id Microsoft.VisualStudio.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  ```

  A Visual Studio (Community or another edition, 2022 or 2026) with that
  workload works as well, and so do the 2022 Build Tools
  (`Microsoft.VisualStudio.2022.BuildTools`). The GNU toolchain
  (`x86_64-pc-windows-gnu`) doesn't: the OpenSSL below is built with MSVC.
- **Git** (`winget install --id Git.Git -e --scope machine`).
- **Node.js 24 LTS.** `OpenJS.NodeJS.LTS` follows whichever release is LTS,
  so install a 24.x with `--version` and keep it there with
  `winget pin add --id OpenJS.NodeJS.LTS --version 24.*`.
- **Python 3.13**, the dev container's version, for all users and on the
  machine `PATH`, with the `py` launcher:
  `winget install --id Python.Python.3.13 -e --scope machine --override "/quiet InstallAllUsers=1 PrependPath=1 Include_launcher=1 InstallLauncherAllUsers=1 Include_test=0"`.
  On Windows it's `python` or `py -3.13`; `python3` is the Microsoft Store's
  alias.
- **WebView2**, the engine the app's window uses. Windows 10 and 11 include
  it; otherwise get it from the
  [WebView2 runtime page](https://developer.microsoft.com/microsoft-edge/webview2/)
  or `winget install --id Microsoft.EdgeWebView2Runtime -e`.
- **OpenSSL 3, from [vcpkg](https://vcpkg.io).** SQLCipher needs it on
  Windows (macOS uses CommonCrypto and Linux uses `libssl-dev`). It must be
  OpenSSL 3: 1.1 and older aren't GPLv3-compatible (see
  [License audit](licenses.md#license-audit)). Use a vcpkg of its own, cloned from
  GitHub; the one that comes with Visual Studio has no classic mode, so
  `vcpkg install` refuses there.

  ```powershell
  git clone https://github.com/microsoft/vcpkg C:\vcpkg
  C:\vcpkg\bootstrap-vcpkg.bat -disableMetrics
  C:\vcpkg\vcpkg install openssl:x64-windows-static-md
  ```

  Then set `OPENSSL_DIR=C:\vcpkg\installed\x64-windows-static-md` and
  `OPENSSL_STATIC=1`, machine-wide or for the user. The build stops with
  "Missing environment variable OPENSSL_DIR" otherwise. Static matters for
  tests and E2E too, not only releases: the app starts its render and import
  helpers with only `LOCALAPPDATA` in their environment, so a DLL-linked build's helpers can't
  find `libcrypto-3-x64.dll` and exit before they serve (the log shows only
  "started --render-helper", a preview never appears, and an import says
  "could not read the file"). After changing `OPENSSL_DIR`,
  `OPENSSL_LIB_DIR` or `OPENSSL_INCLUDE_DIR`, run
  `cargo clean -p libsqlite3-sys`: its build script doesn't watch them, so
  SQLCipher keeps the old link.

  CI builds the OpenSSL the repository pins instead
  (`scripts/windows/vcpkg/vcpkg.json`, see
  [Continuous integration](ci.md)). Nothing reads that
  manifest unless you ask, so the commands above are unchanged. To build
  against the same OpenSSL as CI, bring `C:\vcpkg` up to date
  (`git -C C:\vcpkg pull`, so it holds the pinned commit), install the
  manifest into a folder of its own, and point `OPENSSL_DIR` there (then
  `cargo clean -p libsqlite3-sys`, as above):

  ```powershell
  C:\vcpkg\vcpkg install --x-manifest-root=scripts\windows\vcpkg --x-install-root=C:\hoplodex-openssl --triplet x64-windows-static-md
  # OPENSSL_DIR=C:\hoplodex-openssl\x64-windows-static-md, OPENSSL_STATIC=1
  ```

  Run it from the checkout, not `C:\vcpkg\installed`: a manifest install
  removes whatever its install folder holds that the manifest doesn't name.
- **Long paths:** `LongPathsEnabled` = 1 under
  `HKLM\SYSTEM\CurrentControlSet\Control\FileSystem`, and
  `git config --system core.longpaths true`, for deep `node_modules` and
  `target` paths.
- **Remote access for `-User`:** Remote Desktop on, with Network Level
  Authentication, and open in the firewall; the OpenSSH server
  (`sshd`, built into Server 2025, an optional capability elsewhere)
  started automatically and open on port 22; and the user in the "Remote
  Desktop Users" and "OpenSSH Users" groups. Windows Home has no Remote
  Desktop host, so the script refuses it. SSH is restricted to that group
  with `AllowGroups "openssh users"` in `%ProgramData%\ssh\sshd_config`,
  before its first `Match` block (Windows' `sshd` wants names in lower
  case), so an administrator who isn't in the group can't sign in over
  SSH. The script keeps the old file as `sshd_config.before-hoplodex`.
- **Windows Server:** Server Manager doesn't open at sign-in (its
  scheduled task is off, and the `DoNotOpenAtLogon` policy is set).
- **Auto-logon (`-AutoLogon`, test machines only):** E2E and screenshot
  runs need a desktop session that stays signed in and drawing. Windows
  signs the user in at startup (`AutoAdminLogon`, `DefaultUserName` and
  `DefaultDomainName` under `HKLM\...\Winlogon`). The password is kept
  as the `DefaultPassword` LSA secret, as Sysinternals Autologon does,
  rather than in the registry, where any local user can read it. The
  session never locks, blanks or sleeps when idle: display, sleep and
  hibernate timeouts are off, no password on wakeup, no machine
  inactivity limit, and the user's screen saver is off by policy. The
  user's "Show animations in Windows" is turned on (a bit of
  `UserPreferencesMask`): Windows Server starts with it off, and WebView2
  then reports reduced motion, so `databases`' catalogue-plate test fails. Both
  live in the user's own registry, so if they've never signed in, restart (auto-logon signs them in) and run the script again.
  Avoid connecting with Remote Desktop as that user before a test run: it
  takes over the console session, which stops drawing once you
  disconnect.

**Per user** (`setup-user.ps1`), after a new shell picks up the new `PATH`:

- **rustup** (`winget install --id Rustlang.Rustup -e`, or
  [`rustup-init.exe`](https://win.rustup.rs)), then
  `rustup toolchain install stable --component rustfmt --component clippy`.
  Install it after the build tools, so its default host is
  `x86_64-pc-windows-msvc`.
- **npm 12:** `npm install --global npm@12`. It lands in `%APPDATA%\npm`,
  and Node's own `npm.cmd` hands off to it, for this account only.
- **cargo-nextest and cargo-deny:** `cargo install --locked cargo-nextest cargo-deny`.

## Driving a test machine from Linux, over SSH

**Driving a test machine from Linux, over SSH.** Agents run on Linux and
reach a Windows test machine over SSH as its auto-logon user, which needn't
be an administrator; no Claude session runs on Windows. That account's
shell is `cmd`, which mangles PowerShell's quotes and pipes, so send
PowerShell encoded, as an argument or a heredoc, and end with
`exit $LASTEXITCODE` to get a native command's status back:

```bash
winps() { ssh -n <user>@<windows-host> "powershell -NoProfile -ExecutionPolicy Bypass -EncodedCommand $(printf '$ProgressPreference="SilentlyContinue"\n%s' "${1-$(cat)}" | iconv -t UTF-16LE | base64 -w0)"; }
winps 'cd C:\HoploDex; git fetch -q; git switch <branch>; git pull --ff-only'   # after pushing <branch>
winps <<'PS'
C:\HoploDex\scripts\windows\gui.ps1 'cargo nextest run --manifest-path src-tauri/Cargo.toml --features test-kdf'
exit $LASTEXITCODE
PS
winps <<'PS'
C:\HoploDex\scripts\windows\gui.ps1 'npm run build; if ($LASTEXITCODE) { exit $LASTEXITCODE }; npm run test:e2e'
exit $LASTEXITCODE
PS
scp -r '<user>@<windows-host>:C:/HoploDex/e2e/screenshots-out' e2e/   # results back
```

- **The checkout** is a git clone on the machine's own disk (a network share
  is slow, and changes SQLite's locking and the file-identity check). Code
  goes over through GitHub: push the branch, then pull it there.
- **Directly over SSH:** builds, Vitest, lint and the audits. Building the
  MSI is the exception: its `light.exe` failed over SSH and worked through `gui.ps1`.
  Not with `ssh -t`: an interactive session's console closes when the
  connection drops, and its commands end with it.
- **Through `scripts\windows\gui.ps1`:** `cargo nextest`, and anything that
  opens the app's window: E2E, screenshots, `quit-cleanup.ps1`, the PDF surface
  and window controls checks. An SSH session runs in session 0, which has no
  desktop, so the app gets no window there, and a helper's AppContainer can't
  start: the helper dies at start with `0xC0000142`, which fails every test
  that starts one. GitHub's Windows runners run steps in a desktop session, so
  CI needs none of this. `gui.ps1` runs its PowerShell command
  (in the checkout, or `-WorkingDirectory`) in the auto-logon user's desktop
  session, through a scheduled task, "HoploDex gui", that it registers for
  that user with an interactive logon, not elevated. It prints the command's
  output as it comes and exits with its status, as `scripts/tart-vm.sh gui`
  does on macOS. Interrupting it, or losing the connection, leaves the
  command running, and its log stays in `%LOCALAPPDATA%\HoploDex\gui\<run>\`
  (named at the start). The user must be signed in at the console and not
  disconnected, which auto-logon gives you. Run one of these at a time.

## E2E on Windows

**E2E on Windows:** `npm run test:e2e` runs there (over SSH, through
`gui.ps1`), through the embedded WebDriver server, with no driver to
install, and is isolated from your real data like everywhere else (see
[Test isolation](test-isolation.md)). The real-input tests skip themselves,
by decision (see
[Real keyboard and mouse input](real-input.md)).
`npm run screenshots` runs there too, with the window on the desktop. Its
frame takes some of the 1200×800, so the shots are 1184×761. Full-page shots
grow the window past the bottom of the screen, which Windows allows. So take
before/after pairs on one platform.
With Windows' "Show animations in Windows" off (Settings › Accessibility ›
Visual effects), WebView2 reports reduced motion and `databases`' catalogue-plate
test fails; `setup-system.ps1 -AutoLogon` turns it on.
Windows CI runs the E2E sandboxes on the runner's temp drive (`D:`), so
`scanRoots` in `document-preview.e2e.ts` no longer scans the `C:` temp folder there.
A WebDriver screenshot leaves out the PDF surface (a child web view), so where
Linux takes the X display with `import -window root`, `document-preview`'s pixel checks and
the walk's viewer shots (`shotDisplay`) run `e2e/scripts/window-shot.ps1` on
Windows. It saves the client area of this worker's app window (by its process,
`appPid()`), so a point in the page is the same point in the picture. It asks
the window to paint itself (PrintWindow) rather than copying the screen, so
the parallel workers' windows over it don't matter (#88); the window mustn't
be minimized. macOS does the same with `screencapture -l` on the worker's
window (`macWindowShot` in `e2e/support/screenshots.ts`), which still needs
Screen Recording.
