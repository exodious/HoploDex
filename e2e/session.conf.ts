/**
 * The configuration `npm run session` (e2e/session.ts) opens a `wdio session`
 * with: only what the session needs to reach the E2E app's WebDriver server,
 * and none of wdio.conf.ts. `wdio session open <config> 0` only reads a
 * config's capabilities and connection settings; it runs none of its hooks,
 * builds nothing and starts no app or display. The launcher does all that,
 * in a sandbox (support/sandbox.ts), and passes the port.
 *
 * Don't point `wdio session open` at wdio.conf.ts or at the real binary: this
 * file and the launcher are the way in (DEVELOPMENT.md, "Drive the app with
 * wdio session").
 */
export const config: WebdriverIO.Config = {
  runner: "local",
  framework: "mocha",
  capabilities: [
    {
      browserName: "wry",
      // The embedded server speaks WebDriver classic only (wdio.conf.ts).
      "wdio:enforceWebDriverClassic": true,
    },
  ],
  hostname: "127.0.0.1",
  path: "/",
};
