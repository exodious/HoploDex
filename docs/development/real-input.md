# Real keyboard and mouse input

_Part of the [development docs](../../DEVELOPMENT.md)._

WebDriver's clicks and keys don't reach WebKitGTK the way a person's do, so
some bugs a person sees don't reproduce under them. Examples: a focus ring
that shows or doesn't (`:focus-visible` after a script moves focus), or a
dialog whose layout only corrects itself on the next real key press. When a
report says "after a mouse click" or "when I press Tab" and a spec can't
reproduce it, send real X11 input instead. `e2e/scripts/x11-input.py` sends
it through XTest to the worker's Xvfb display (`DISPLAY`), and `e2e/support/realInput.ts`
wraps it for specs:

```ts
import { realClick, realKey } from "../support/realInput";

await realClick("button.hd-db-menu"); // a real pointer click
await realClick('[role="menuitem"]*=Restore from a backup'); // WebdriverIO's text match
await realKey("Tab"); // X keysym names
await realKey("Shift_L+Tab"); // + for a chord
```

It needs `libX11`, `libXtst` and `python3` (all in the dev container) and
runs on Linux only. The first call moves the pointer once to find where the
window sits on the screen. Keys go to the window under the pointer.

Each call returns when the page has seen the input end (a `keyup`, a
`pointerup` or `mouseup`, or the pointer's arrival, counted by listeners
`realInput.ts` installs on the window) and the app has settled, as `settle()`
does for any step, except that it doesn't wait for animations: a test may look
at one the input started (`databases`' owl blinking). There is no fixed pause after
an event. Where the page can't see the input, it waits 150 ms as before: keys
sent while the page has no focus (F6 into the PDF surface), and an event the
page never reports within a second. The script runs as `x11-input.py serve`,
one process for the worker's life, taking a command a line and answering `ok`
once the X server has processed it, so no Python starts for each key. A test
that looks at the page after real input needn't pause for it; one that looks
at something the input only starts (a window closing, a file written) waits
for that, as with any other step.

A test that sends real input starts with `skipWithoutRealInput(this)` (from
`realInput.ts`, in an `async function`, not an arrow, for Mocha's `this`),
which skips it on other platforms. A block whose tests all do calls it from a
`before` hook. So does a test that needs what such a test did (a record it
added, a database it left open). Off Linux, `realClick` and `realKey` throw
if a test reaches them without it.

Real input stays on Linux only, by decision (#27): Windows and macOS skip
these tests rather than port the helper. Two kinds need it. WebKitGTK
regressions (a focus ring, a first layout) only matter on Linux. The
keyboard-only flows (`cartridges-actions`, `regulated-items`, the `accessories-*`
files) need it on every platform, because
the embedded WebDriver's key presses are synthetic: they don't move focus on
Tab or type text. So use real input only for what needs it, and keep it out
of anything else's way. A check that needs it goes in a test of its own,
which leaves the database as it found it, and the other tests don't depend
on it. `backups`' restore-dialog regressions are an example. The screenshot walk
uses no real input, so it runs everywhere.

A second exception runs on every OS: `passphrase-dialog`'s tests type into the
native passphrase dialog, which no WebDriver can reach, with real keys:
XTest on Linux (`realInput.ts`'s `realKeyOutsidePage`), `real_input.swift`'s
`key-to` to the app's process on macOS (`macInput.ts`), and
`e2e/scripts/dialog-input.ps1` on Windows, which posts the keys to the app's
dialog itself, since another worker's app can have the foreground (as macOS
addresses its keys to the process, #122), all through `e2e/support/dialogInput.ts`.
The app runs with `HOPLODEX_E2E_PASSPHRASE_DIALOG=native` there, and the spec
reads what the dialog shows from its log (`HOPLODEX_E2E_PASSPHRASE_LOG`: each
prompt, refusal, view and close, never a passphrase). Every other spec answers
the dialog from a queue file (`answerPassphrase` in `ui.ts`, through
`HOPLODEX_E2E_PASSPHRASES`), as `E2eConsent` answers the consent dialog. A
click that opens a passphrase dialog mustn't wait for the app to settle: its
command is busy until the dialog is answered. On Linux the spec also scans
the WebKit web process's memory for the passphrase (`memoryScan.ts`), which
the worker may read, being the app's ancestor.

One exception runs on macOS only: `databases`' quit test (#107) presses ⌘Q, chooses
the app menu's Quit and sends the quit Apple Event the Dock's Quit sends (to
the sandboxed app's process only) with `e2e/support/macInput.ts`, which
compiles `src-tauri/examples/real_input.swift`. Every one of its functions is addressed to
the test's own process (⌘Q by `key-to`, which posts the key with
`CGEvent.postToPid`; the others through Accessibility and Apple Events), because
the suite's workers run apps side by side and a new one takes the front: a
global `key` goes to whichever app is frontmost when it arrives (#122). `locking` sends the quit event with
a log out's reason (`kAEReallyLogOut`), which must end the app at once with
its unsaved input kept as pending changes. It needs Accessibility for
whatever runs the suite (as `quit-cleanup.py` does) and is skipped on the
other platforms.

The PDF surface and window controls checks do not use that helper. They are
Rust examples run outside WebDriver, and each OS gives them its own input
script in `src-tauri/examples/`: `real_input.py` (Linux; it wraps
`e2e/scripts/x11-input.py` and adds the right button, the wheel and the mouse's
back and forward buttons), `real_input.swift` (macOS, compiled by the check's
script; the terminal needs Accessibility access) and `real_input.ps1`
(Windows). They were `pdf_surface_input.*` before specs/008-hardening-batch
renamed them, because a second check uses them. Each file's header lists its
commands (`click`, `rclick`, `move`, `key`, `xbutton` and more); a check passes
the one for its OS to the example with `--input`.

To track down a bug of this kind, write a throwaway spec that opens the
screen with real input and logs what you need from the page with
`browser.execute`: `getBoundingClientRect()` of the elements involved,
`document.activeElement.matches(":focus-visible")`, and, to catch a first
layout that differs from the settled one, a snapshot taken from a
`MutationObserver` the moment the element appears. Then change one thing
at a time, by injecting a `<style>` before opening the screen, and compare.
Run it with `npm run test:e2e -- --screenshots=<dir> --spec <file>`, look
at the screenshots, and delete the spec before committing.
