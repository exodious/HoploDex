# Drive the app with wdio session

_Part of the [development docs](../../DEVELOPMENT.md)._

`wdio session` (WebdriverIO 10) keeps one session alive across short shell
commands, so a person or a coding agent can drive the app step by step: look
at it, click, run code, and turn the steps into a spec. Start it only through
the sandboxed launcher:

```bash
npm run build                      # the E2E build embeds dist/, as for test:e2e
npm run session                    # a first run: no databases
npm run session -- --seed          # with the human-testing collection (see Human testing)
npm run session -- --port 4600     # another WebDriver port (default 4545; the E2E workers use 4445 and up)
```

In the [development container](dev-container.md) the
launcher, the app and the `wdio session` commands must all run in the same
container, since each `scripts/dev-container.sh` call starts a new one: start
a shell there (`scripts/dev-container.sh`), run `npm run session` in the
background and the commands below in the foreground.

The launcher builds the E2E app, as `onPrepare` does (`e2e/support/build.ts`),
creates the same throwaway sandbox each spec gets (`e2e/support/sandbox.ts`;
see [Test isolation](test-isolation.md)), starts the app in it on a virtual display (Linux;
`GDK_BACKEND=x11` and no `WAYLAND_DISPLAY`, as `run-e2e.mjs` has it), and runs
`wdio session open e2e/session.conf.ts 0` against the app's embedded WebDriver
server (WebDriver classic only, so `--no-bidi`). It then stays in the
foreground, printing the sandbox's path and, with `--seed`, which databases
to unlock and the passphrase. Ctrl+C, SIGTERM, `wdio session close` or the
session's idle timeout (2 hours) end it: it closes the session, kills the app,
stops the display and removes the sandbox. In another terminal:

```bash
npx wdio session snapshot --interactive            # the page as text; what is clickable
npx wdio session click e3                          # a ref from the snapshot, or a selector such as 'button=Open…'
echo '{"passphrase":"<passphrase>"}' >> <sandbox>/passphrases.queue   # the native passphrase dialog's answer, before Open…
npx wdio session exec -e "await browser.getTitle()"
npx wdio session screenshot
npx wdio session export --out e2e/specs/<name>.e2e.ts
npx wdio session close                             # also ends the launcher
```

`npx wdio session --help` and `<action> --help` list every action. A coding
agent gets the same loop from the `wdio-session` skill
(`.agents/skills/wdio-session/`, written by `npx wdio session skill
--install .` from the installed `@wdio/session`, so rerun that after
upgrading WebdriverIO; it isn't in `skills-lock.json`, which is the `skills`
CLI's), which says to `open` a browser or a binary itself; the rules here
override that:

- **Always `npm run session`.** Never `wdio session open tauri <app>` or
  `open <app>`: they launch the real binary outside the sandbox, on your real
  databases, recent list and keyring. Don't `open ./e2e/wdio.conf.ts` either:
  `open` runs none of a config's hooks (verified), so nothing would start the
  app. `e2e/session.conf.ts`, which the launcher opens, holds only the app's
  WebDriver endpoint.
- **Refs or selectors.** `click e3` acts on a ref from the latest snapshot,
  and the export records it as a stable selector, usually
  `role/<role>[name="<accessible name>"]`. A selector works too:
  `aria/<accessible name>`, `button=<text>`, or CSS. A selector that matches
  more than one element fails (`$` is strict); narrow it.
- The artifacts (snapshots, screenshots, exports without `--out`) go to
  `.wdio/session/default/`, git-ignored. The window is on the virtual display
  on Linux: use `screenshot`.
- The sandbox starts with the harness's defaults: the in-memory keyring, the
  native "open in another app" confirmation answered "Open", the native
  passphrase dialog answered from `<sandbox>/passphrases.queue` (one JSON
  answer a line, as `answerPassphrase` in `e2e/support/ui.ts` writes them; with
  nothing queued it cancels), PDF preview on, the real idle minute. A spec's own settings (PDF preview off, the
  locking spec's shortened minute) aren't offered; `exec` can't set the app's
  environment.
- In the macOS VM, `scripts/tart-vm.sh ssh` splits its arguments again on
  the VM's side, so give a selector with spaces or quotes as one plain
  single-quoted argument: `scripts/tart-vm.sh ssh npx wdio session fill
  'aria/Name' 'Club armory'`.

An exported spec is a starting point, not a test, and needs review before it
joins the suite. It records only your steps, in WebdriverIO's style, so
before `npm run test:e2e -- --spec <file>` runs it:

1. Each spec starts at a first run in a new sandbox (`HOPLODEX_E2E_DOCUMENTS`
   and the rest), so begin with the harness's `createDatabase()` and
   `waitForCollection()`, or `unlock(passphrase)` for a seeded collection,
   from `e2e/support/ui.ts`, instead of the typed-in paths and passphrase.
   A spec that needs the seeded collection must be named in `beforeSession`
   in `wdio.conf.ts` (only the screenshot walk, `document-preview`,
   `document-open-elsewhere`, `document-open-setting` and `session-isolation` are), and reads the passphrase from `HOPLODEX_E2E_SEED_PASSPHRASE`.
2. Import `$`, `browser` and `expect` from `../support/ui`, like the other
   specs, not from `@wdio/globals`.
3. Replace the recorded selectors where a helper exists (`clickButton`,
   `fill`, `goTo`, and so on), call `settle()` after actions the way the
   helpers do (E2E steps don't sleep), and give the suite and test real
   names (the export names both `default`).
4. Run `npx prettier --write` and `npx eslint` on it; the export uses single
   quotes and four spaces.
