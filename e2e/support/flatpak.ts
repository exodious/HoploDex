import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
// A type only: app.ts imports this module (its default launcher under
// HOPLODEX_E2E_FLATPAK=1), so nothing here may import app.ts at run time, nor
// sandbox.ts or build.ts, which import it (011 research.md §16).
import type { Launcher } from "./app";

/**
 * The automated Flatpak check's support (011 FR-021, 011 SC-007; 011
 * research.md §16): the check variant's build and install, the arguments the
 * app is started with, the stand-in services it talks to, and the probes that
 * look at the sandbox from outside.
 *
 * Isolation (011 SC-007): every `flatpak` command here runs in the dev
 * container (`scripts/dev-container.sh --flatpak`), against the installation
 * `scripts/flatpak/env.sh` accepted (`FLATPAK_USER_DIR`) and its private system
 * bus, and `flatpak run` gets the run's throwaway home as `HOME`, so no
 * `~/.var/app` is made in the real home. {@link requireCheckEnvironment}
 * refuses to do any of it otherwise. `scripts/flatpak/check.sh` is the way in.
 */

export const FLATPAK_APP_ID = "io.github.exodious.HoploDex";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

/** Where the check's logs go (011 contracts/flatpak-bundle.md "Output locations"). */
export const CHECK_LOG_DIR = path.join(repoRoot, "flatpak", "out", "check");

/** Whether this run is the Flatpak check's (`HOPLODEX_E2E_FLATPAK=1`). */
export function flatpakMode(): boolean {
  return process.env.HOPLODEX_E2E_FLATPAK === "1";
}

/** Set by check.sh once it has built and installed the check variant, so the
 * E2E run doesn't build it again. */
const INSTALLED_VARIABLE = "HOPLODEX_FLATPAK_CHECK_INSTALLED";

/** The remote the check variant is installed from (a local repository). */
const CHECK_REMOTE = "hoplodex-check";

/** The four folders the `e2e` build is given (src-tauri/src/app_dirs.rs). A
 * caller's extra variables can't replace them. */
const FOLDER_VARIABLES = [
  "HOPLODEX_E2E_CONFIG_HOME",
  "HOPLODEX_E2E_CACHE_HOME",
  "HOPLODEX_E2E_DOCUMENTS",
  "HOPLODEX_E2E_HOME",
] as const;

/** `~/.var/app/<id>` under `home`: the app's data in a Flatpak. */
export function appDataDir(home: string): string {
  return path.join(home, ".var", "app", FLATPAK_APP_ID);
}

export interface RunOptions {
  /** The run's throwaway home, an absolute path. */
  home: string;
  /** The Documents folder as read inside the sandbox ({@link documentsDir}). */
  documents: string;
  /** Where the app's WebDriver server listens. */
  port: number;
  /** More variables for the app, none of them the four folders. */
  extraEnv?: Record<string, string>;
}

/**
 * What to run for the app: `flatpak <args>`, with `env` laid over this
 * process's environment. The app's permissions are the shipped ones plus
 * `--share=network`, which the test runner outside the sandbox needs to reach
 * the WebDriver port; nothing else is added (011 research.md §16, 011
 * SC-007). The `e2e` build's folders are the paths a shipped Flatpak uses
 * under the run's home, and `HOME` is the throwaway home, never the real one.
 * Pure: it starts nothing.
 */
export function flatpakRunArgs(options: RunOptions): {
  args: string[];
  env: Record<string, string>;
} {
  const { home, documents, port, extraEnv = {} } = options;
  if (!path.isAbsolute(home)) {
    throw new Error(`the Flatpak check needs an absolute throwaway home, not "${home}"`);
  }
  if (!path.isAbsolute(documents)) {
    throw new Error(`the Flatpak check needs an absolute Documents folder, not "${documents}"`);
  }
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(`"${port}" is not a port`);
  }
  const data = appDataDir(home);
  const variables: Record<string, string> = {};
  for (const [name, value] of Object.entries(extraEnv)) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) {
      throw new Error(`"${name}" is not a variable name`);
    }
    if ((FOLDER_VARIABLES as readonly string[]).includes(name) || name === "TAURI_WEBDRIVER_PORT") {
      continue;
    }
    variables[name] = value;
  }
  variables.HOPLODEX_E2E_CONFIG_HOME = path.join(data, "config");
  variables.HOPLODEX_E2E_CACHE_HOME = path.join(data, "cache");
  variables.HOPLODEX_E2E_DOCUMENTS = documents;
  variables.HOPLODEX_E2E_HOME = home;
  variables.TAURI_WEBDRIVER_PORT = String(port);
  return {
    args: [
      "run",
      "--share=network",
      ...Object.entries(variables).map(([name, value]) => `--env=${name}=${value}`),
      FLATPAK_APP_ID,
    ],
    env: { HOME: home },
  };
}

// ---------------------------------------------------------------------------
// The environment the check runs in
// ---------------------------------------------------------------------------

/**
 * Refuses to go on unless check.sh (which sources env.sh: the host guard, the
 * accepted `FLATPAK_USER_DIR` and the private system bus) started this run.
 * Without it a `flatpak` command here would use whatever installation the
 * environment names, which on a host is the developer's own (011 SC-007).
 */
export function requireCheckEnvironment(): { state: string } {
  const missing = [
    "FLATPAK_USER_DIR",
    "HOPLODEX_FLATPAK_STATE",
    "DBUS_SYSTEM_BUS_ADDRESS",
    "HOPLODEX_FLATPAK_CHECK",
  ].filter((name) => !process.env[name]);
  if (missing.length > 0) {
    throw new Error(
      `The Flatpak check runs through \`npm run test:flatpak\` (scripts/flatpak/check.sh) in ` +
        `\`scripts/dev-container.sh --flatpak\`, which sets up its throwaway Flatpak installation ` +
        `and buses first; ${missing.join(", ")} ${missing.length === 1 ? "is" : "are"} not set.`,
    );
  }
  return { state: process.env.HOPLODEX_FLATPAK_STATE as string };
}

function ensureLogDir(): string {
  fs.mkdirSync(CHECK_LOG_DIR, { recursive: true });
  return CHECK_LOG_DIR;
}

/** The environment for a `flatpak run` of the app under `home`: the throwaway
 * home, and none of the sandbox's `XDG_*` folders, so that Flatpak reads and
 * makes `~/.var/app` and `~/.config/user-dirs.dirs` under that home only. */
function runProcessEnv(home: string, extra: Record<string, string | undefined> = {}) {
  const env: NodeJS.ProcessEnv = { ...process.env, HOME: home };
  for (const name of ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"]) {
    delete env[name];
  }
  for (const [name, value] of Object.entries(extra)) {
    if (value === undefined) delete env[name];
    else env[name] = value;
  }
  return env;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// ---------------------------------------------------------------------------
// Building and installing the check variant
// ---------------------------------------------------------------------------

/** Runs one build step, appending its output to the build log; throws with the
 * log's tail when it fails. Returns what the step printed on stdout. */
function runLogged(label: string, command: string, args: string[], env = process.env): string {
  const log = path.join(ensureLogDir(), "build.log");
  fs.appendFileSync(log, `\n==> ${label}\n$ ${command} ${args.join(" ")}\n`);
  console.log(`flatpak check: ${label}`);
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env,
    encoding: "utf-8",
    maxBuffer: 1 << 28,
  });
  fs.appendFileSync(log, `${result.stdout ?? ""}${result.stderr ?? ""}`);
  if (result.status !== 0) {
    const tail = `${result.stdout ?? ""}${result.stderr ?? ""}`.trimEnd().split("\n").slice(-30);
    throw new Error(
      `Flatpak check: "${label}" failed (exit ${result.status ?? result.signal}); full log: ${log}\n${tail.join("\n")}`,
    );
  }
  return result.stdout ?? "";
}

function flatpakInfoInstalled(): boolean {
  return spawnSync("flatpak", ["info", "--user", FLATPAK_APP_ID], { stdio: "ignore" }).status === 0;
}

/**
 * Builds the check variant (scripts/flatpak/checkManifest.mjs derives its
 * manifest from the shipped one) with `flatpak-builder` into a build directory
 * and repository of its own under the state volume, and installs it into the
 * container's user installation from that repository. The shipped build's
 * (`build.sh`'s) are not touched, and it is never written to a `.flatpak` file
 * (011 research.md §16). Returns whether this call installed it: not when
 * check.sh already has (it sets `HOPLODEX_FLATPAK_CHECK_INSTALLED=1`).
 */
export function buildAndInstallCheck(): { installedHere: boolean } {
  const { state } = requireCheckEnvironment();
  if (process.env[INSTALLED_VARIABLE] === "1") {
    if (!flatpakInfoInstalled()) {
      throw new Error(`${INSTALLED_VARIABLE}=1, but ${FLATPAK_APP_ID} is not installed`);
    }
    return { installedHere: false };
  }
  const arch = spawnSync("flatpak", ["--default-arch"], { encoding: "utf-8" }).stdout.trim();
  const checkDir = path.join(state, "check");
  const buildDir = path.join(checkDir, "build-dir");
  const repo = path.join(checkDir, "repo");
  fs.mkdirSync(path.join(state, "state"), { recursive: true });
  const manifest = runLogged("derive the check manifest", process.execPath, [
    path.join(repoRoot, "scripts", "flatpak", "checkManifest.mjs"),
  ]).trim();
  // --disable-rofiles-fuse: the dev container has no /dev/fuse (as build.sh).
  const builder = (...more: string[]) => [
    "--user",
    `--arch=${arch}`,
    `--state-dir=${path.join(state, "state")}`,
    "--disable-rofiles-fuse",
    ...more,
  ];
  runLogged(
    "flatpak-builder --download-only (network)",
    "flatpak-builder",
    builder("--download-only", "--force-clean", buildDir, manifest),
  );
  runLogged(
    "flatpak-builder --disable-download (no network)",
    "flatpak-builder",
    builder("--disable-download", "--force-clean", `--repo=${repo}`, buildDir, manifest),
  );
  if (flatpakInfoInstalled()) {
    runLogged("uninstall the build that is installed", "flatpak", [
      "uninstall",
      "--user",
      "--noninteractive",
      FLATPAK_APP_ID,
    ]);
  }
  runLogged("add the check's local remote", "flatpak", [
    "remote-add",
    "--user",
    "--if-not-exists",
    "--no-gpg-verify",
    CHECK_REMOTE,
    repo,
  ]);
  runLogged("install the check variant", "flatpak", [
    "install",
    "--user",
    "--reinstall",
    "--noninteractive",
    "--assumeyes",
    CHECK_REMOTE,
    FLATPAK_APP_ID,
  ]);
  return { installedHere: true };
}

/** Ends the app's sandbox, uninstalls the check variant and drops its remote.
 * Each step is tried whatever the one before did. */
export function uninstallCheck(): void {
  requireCheckEnvironment();
  const log = path.join(ensureLogDir(), "build.log");
  for (const args of [
    ["kill", FLATPAK_APP_ID],
    ["uninstall", "--user", "--noninteractive", FLATPAK_APP_ID],
    ["remote-delete", "--user", "--force", CHECK_REMOTE],
  ]) {
    const result = spawnSync("flatpak", args, { encoding: "utf-8" });
    fs.appendFileSync(
      log,
      `\n$ flatpak ${args.join(" ")} -> ${result.status}\n${result.stdout}${result.stderr}`,
    );
  }
}

// ---------------------------------------------------------------------------
// The run: the throwaway home, the buses and the stand-in services
// ---------------------------------------------------------------------------

interface Mock {
  /** The bus name it owns. */
  name: string;
  process: ChildProcess;
}

interface FlatpakRun {
  home: string;
  documents: string;
  busDir: string;
  sessionBus: { address: string; pid: number };
  logind: Mock;
  screenSavers: Mock[];
  secrets: Mock | undefined;
  portal: Mock | undefined;
  launches: number;
}

const stateKey = Symbol.for("io.github.exodious.HoploDex.e2e.flatpakRun");
const globals = globalThis as { [stateKey]?: FlatpakRun };

/** The run `startFlatpakRun` made in this process. It lives on `globalThis`,
 * for the reason support/app.ts gives. */
export function currentRun(): FlatpakRun {
  const run = globals[stateKey];
  if (!run) throw new Error("no Flatpak run: startFlatpakRun() comes first");
  return run;
}

/** Runs `gdbus` against the bus at `address`; throws on a failure. */
function gdbus(address: string, args: string[]): string {
  const result = spawnSync("gdbus", ["call", "--address", address, ...args], { encoding: "utf-8" });
  if (result.status !== 0) {
    throw new Error(
      `gdbus ${args.join(" ")} failed: ${result.stderr.trim() || result.stdout.trim()}`,
    );
  }
  return result.stdout.trim();
}

function nameHasOwner(address: string, name: string): boolean {
  try {
    return gdbus(address, [
      "--dest",
      "org.freedesktop.DBus",
      "--object-path",
      "/org/freedesktop/DBus",
      "--method",
      "org.freedesktop.DBus.NameHasOwner",
      name,
    ]).includes("true");
  } catch {
    return false;
  }
}

interface MockSpec {
  bus: "system" | "session";
  address: string;
  name: string;
  /** A dbusmock template, or the object's path and interface. */
  template?: string;
  path?: string;
  iface?: string;
}

/** Starts a python-dbusmock stand-in owning `spec.name`, logging to
 * flatpak/out/check/, and waits until the bus says it does. */
async function startMock(spec: MockSpec): Promise<Mock> {
  if (nameHasOwner(spec.address, spec.name)) {
    throw new Error(
      `${spec.name} already has an owner on the ${spec.bus} bus; stop the other stand-in first`,
    );
  }
  const fd = fs.openSync(path.join(ensureLogDir(), `standin-${spec.name}.log`), "w");
  const args = [
    "-m",
    "dbusmock",
    spec.bus === "system" ? "--system" : "--session",
    ...(spec.template
      ? ["-t", spec.template]
      : [spec.name, spec.path as string, spec.iface as string]),
  ];
  const env = { ...process.env };
  env[spec.bus === "system" ? "DBUS_SYSTEM_BUS_ADDRESS" : "DBUS_SESSION_BUS_ADDRESS"] =
    spec.address;
  const child = spawn("python3", args, { env, stdio: ["ignore", fd, fd] });
  fs.closeSync(fd);
  const deadline = Date.now() + 15000;
  for (;;) {
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error(
        `the ${spec.name} stand-in exited at start; see flatpak/out/check/standin-${spec.name}.log`,
      );
    }
    if (nameHasOwner(spec.address, spec.name)) return { name: spec.name, process: child };
    if (Date.now() > deadline) {
      child.kill("SIGKILL");
      throw new Error(`the ${spec.name} stand-in never took its name`);
    }
    await sleep(50);
  }
}

async function stopMock(mock: Mock | undefined): Promise<void> {
  if (!mock) return;
  const child = mock.process;
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = new Promise<void>((resolve) => child.once("exit", () => resolve()));
  child.kill("SIGTERM");
  const timer = setTimeout(() => child.kill("SIGKILL"), 3000);
  await exited;
  clearTimeout(timer);
}

const SCREEN_SAVERS = [
  { name: "org.gnome.ScreenSaver", path: "/org/gnome/ScreenSaver" },
  // platform/linux.rs listens to either name.
  { name: "org.freedesktop.ScreenSaver", path: "/org/freedesktop/ScreenSaver" },
] as const;

/**
 * A private session bus for the run, with no service activation of the
 * container's own (the container's gnome-keyring would otherwise answer for
 * `org.freedesktop.secrets` once the stand-in is stopped) except Flatpak's two:
 * the session helper and the Flatpak portal, which WebKit's own sandbox asks
 * for (`flatpak-spawn --sandbox`, 011 research.md §9).
 */
function startSessionBus(busDir: string): { address: string; pid: number } {
  const services = path.join(busDir, "services");
  fs.mkdirSync(services, { recursive: true });
  for (const file of [
    "org.freedesktop.Flatpak.service",
    "org.freedesktop.portal.Flatpak.service",
  ]) {
    const source = path.join("/usr/share/dbus-1/services", file);
    if (!fs.existsSync(source)) continue;
    const text = fs.readFileSync(source, "utf-8").replace(/^SystemdService=.*\n/m, "");
    fs.writeFileSync(path.join(services, file), text);
  }
  const socket = path.join(busDir, "session_bus_socket");
  fs.writeFileSync(
    path.join(busDir, "session.conf"),
    `<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
  "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=${socket}</listen>
  <servicedir>${services}</servicedir>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
`,
  );
  const result = spawnSync(
    "dbus-daemon",
    [
      `--config-file=${path.join(busDir, "session.conf")}`,
      "--fork",
      "--print-address",
      "--print-pid",
    ],
    { encoding: "utf-8" },
  );
  const [address, pid] = result.stdout.split("\n");
  if (result.status !== 0 || !address || !/^\d+$/.test(pid ?? "")) {
    throw new Error(`could not start the run's session bus: ${result.stderr}`);
  }
  return { address, pid: Number(pid) };
}

export interface StartRunOptions {
  /** The E2E sandbox's directory (`sandboxPath()`); the run's home is made inside it. */
  sandbox: string;
  /** Start the screen-saver stand-ins (the default). Without them the app finds
   * no owner for either name, and the setting shows as unavailable (011 US2-4). */
  screenSavers?: boolean;
}

/**
 * Starts the run: its throwaway home (with a `user-dirs.dirs` naming a
 * Documents folder inside it), the private session bus, and the stand-in
 * services: logind on env.sh's private system bus (with a `session/auto`
 * object), and `org.gnome.ScreenSaver`, `org.freedesktop.ScreenSaver`,
 * `org.freedesktop.secrets` and `org.freedesktop.portal.Desktop` on the session
 * bus. Then reads the Documents folder inside the sandbox and points this
 * process's `HOPLODEX_E2E_*` variables at the paths the app will see, so a
 * spec reads the same paths the app writes. The check variant must be
 * installed (011 research.md §16).
 */
export async function startFlatpakRun(options: StartRunOptions): Promise<FlatpakRun> {
  requireCheckEnvironment();
  if (globals[stateKey]) throw new Error("a Flatpak run is already started in this process");
  if (!path.isAbsolute(options.sandbox)) throw new Error("the sandbox directory must be absolute");
  const home = path.join(options.sandbox, "home");
  fs.mkdirSync(path.join(home, "Documents"), { recursive: true });
  fs.mkdirSync(path.join(home, ".config"), { recursive: true });
  // Flatpak shows the host's ~/.config/user-dirs.dirs inside the sandbox at its
  // own config folder, so `xdg-user-dir` there reads this one.
  fs.writeFileSync(
    path.join(home, ".config", "user-dirs.dirs"),
    `XDG_DOCUMENTS_DIR="${path.join(home, "Documents")}"\n`,
  );
  const busDir = path.join(options.sandbox, "bus");
  fs.mkdirSync(busDir, { recursive: true });

  const system = process.env.DBUS_SYSTEM_BUS_ADDRESS as string;
  const sessionBus = startSessionBus(busDir);
  const run: FlatpakRun = {
    home,
    documents: path.join(home, "Documents"),
    busDir,
    sessionBus,
    logind: undefined as unknown as Mock,
    screenSavers: [],
    secrets: undefined,
    portal: undefined,
    launches: 0,
  };
  globals[stateKey] = run;
  try {
    run.logind = await startMock({
      bus: "system",
      address: system,
      name: "org.freedesktop.login1",
      template: "logind",
    });
    // The session the app listens to: logind resolves `auto` from the caller
    // (platform/linux.rs).
    gdbus(system, [
      "--dest",
      "org.freedesktop.login1",
      "--object-path",
      "/org/freedesktop/login1",
      "--method",
      "org.freedesktop.DBus.Mock.AddSession",
      "auto",
      "seat0",
      String(os.userInfo().uid),
      os.userInfo().username,
      "true",
    ]);
    if (options.screenSavers !== false) await startScreenSavers();
    run.secrets = await startMock({
      bus: "session",
      address: sessionBus.address,
      name: "org.freedesktop.secrets",
      path: "/org/freedesktop/secrets",
      iface: "org.freedesktop.Secret.Service",
    });
    run.portal = await startMock({
      bus: "session",
      address: sessionBus.address,
      name: "org.freedesktop.portal.Desktop",
      path: "/org/freedesktop/portal/desktop",
      iface: "org.freedesktop.portal.OpenURI",
    });
    run.documents = documentsDir(home);
  } catch (error) {
    await endFlatpakRun();
    throw error;
  }

  // What specs read, as the app will see it (the home is the same path on both
  // sides of the sandbox).
  const data = appDataDir(home);
  const files = path.join(home, ".hoplodex-check");
  fs.mkdirSync(files, { recursive: true });
  process.env.HOPLODEX_E2E_CONFIG_HOME = path.join(data, "config");
  process.env.HOPLODEX_E2E_CACHE_HOME = path.join(data, "cache");
  process.env.HOPLODEX_E2E_DOCUMENTS = run.documents;
  process.env.HOPLODEX_E2E_HOME = home;
  process.env.HOPLODEX_E2E_KEYRING_FILE = path.join(files, "keyring.json");
  process.env.HOPLODEX_E2E_OPENED_LOG = path.join(files, "opened.log");
  process.env.HOPLODEX_E2E_CONSENT_LOG = path.join(files, "consent.log");
  return run;
}

/** Ends the app, the stand-ins and the session bus, and forgets the run. The
 * home is part of the E2E sandbox, which `removeSandbox()` deletes. */
export async function endFlatpakRun(): Promise<void> {
  const run = globals[stateKey];
  if (!run) return;
  delete globals[stateKey];
  await killApp();
  await Promise.all([run.logind, run.secrets, run.portal, ...run.screenSavers].map(stopMock));
  try {
    process.kill(run.sessionBus.pid, "SIGTERM");
  } catch {
    // already gone
  }
}

/** Starts (again) the two screen-saver stand-ins. */
export async function startScreenSavers(): Promise<void> {
  const run = currentRun();
  if (run.screenSavers.length > 0) return;
  for (const saver of SCREEN_SAVERS) {
    run.screenSavers.push(
      await startMock({
        bus: "session",
        address: run.sessionBus.address,
        name: saver.name,
        path: saver.path,
        iface: saver.name,
      }),
    );
  }
}

/** Stops the screen-saver stand-ins, so the next start of the app finds no
 * owner for either name (011 US2-4). */
export async function stopScreenSavers(): Promise<void> {
  const run = currentRun();
  const savers = run.screenSavers.splice(0);
  await Promise.all(savers.map(stopMock));
}

/** Stops the Secret Service stand-in. */
export async function stopSecrets(): Promise<void> {
  const run = currentRun();
  const secrets = run.secrets;
  run.secrets = undefined;
  await stopMock(secrets);
}

const SYSTEM_LOGIND = [
  "--dest",
  "org.freedesktop.login1",
  "--object-path",
  "/org/freedesktop/login1",
] as const;

function emit(address: string, dest: string[], iface: string, signal: string, value: boolean) {
  gdbus(address, [
    ...dest,
    "--method",
    "org.freedesktop.DBus.Mock.EmitSignal",
    iface,
    signal,
    "b",
    `[<${value}>]`,
  ]);
}

/** logind's `PrepareForSleep(starting)` (true: going to sleep; false: awake). */
export function emitPrepareForSleep(starting: boolean): void {
  emit(
    process.env.DBUS_SYSTEM_BUS_ADDRESS as string,
    [...SYSTEM_LOGIND],
    "org.freedesktop.login1.Manager",
    "PrepareForSleep",
    starting,
  );
}

/** logind's `PrepareForShutdown(starting)`. */
export function emitPrepareForShutdown(starting: boolean): void {
  emit(
    process.env.DBUS_SYSTEM_BUS_ADDRESS as string,
    [...SYSTEM_LOGIND],
    "org.freedesktop.login1.Manager",
    "PrepareForShutdown",
    starting,
  );
}

/** The screen saver's `ActiveChanged(active)` (true: the screen locked), from
 * the stand-in for `name` (GNOME's by default; the app takes either). */
export function emitActiveChanged(
  active: boolean,
  name: (typeof SCREEN_SAVERS)[number]["name"] = "org.gnome.ScreenSaver",
): void {
  const saver = SCREEN_SAVERS.find((s) => s.name === name);
  if (!saver) throw new Error(`no such screen saver: ${name}`);
  emit(
    currentRun().sessionBus.address,
    ["--dest", saver.name, "--object-path", saver.path],
    saver.name,
    "ActiveChanged",
    active,
  );
}

export interface Inhibitor {
  what: string;
  who: string;
  why: string;
  mode: string;
}

/** logind's inhibitors that are still held. The stand-in's `Inhibit` keeps the
 * other end of each descriptor and drops the inhibitor once the app closes its
 * end, so this shows when the app has let go (011 research.md §6). */
export function inhibitors(): Inhibitor[] {
  const out = gdbus(process.env.DBUS_SYSTEM_BUS_ADDRESS as string, [
    ...SYSTEM_LOGIND,
    "--method",
    "org.freedesktop.login1.Manager.ListInhibitors",
  ]);
  return [
    ...out.matchAll(
      /\('((?:[^'\\]|\\.)*)', '((?:[^'\\]|\\.)*)', '((?:[^'\\]|\\.)*)', '((?:[^'\\]|\\.)*)', (?:uint32 )?\d+, (?:uint32 )?\d+\)/g,
    ),
  ].map(([, what, who, why, mode]) => ({ what, who, why, mode }));
}

// ---------------------------------------------------------------------------
// Starting the app
// ---------------------------------------------------------------------------

/** Variables of this process the app is given as they are, besides every
 * `HOPLODEX_E2E_*` one the `e2e` build reads: `flatpak run` passes none of the
 * caller's environment on. */
const APP_PASSTHROUGH = [
  "GDK_BACKEND",
  "LIBGL_ALWAYS_SOFTWARE",
  "WEBKIT_DISABLE_COMPOSITING_MODE",
  "WEBKIT_DISABLE_DMABUF_RENDERER",
  "GSK_RENDERER",
  "RUST_LOG",
  "RUST_BACKTRACE",
];

/** The `HOPLODEX_E2E_*` variables the `e2e` build reads besides the four
 * folders (src-tauri/src), which this process's sandbox setup has set. */
const APP_E2E_VARIABLES = [
  "HOPLODEX_E2E_KEYRING_FILE",
  "HOPLODEX_E2E_OPENED_LOG",
  "HOPLODEX_E2E_CONSENT",
  "HOPLODEX_E2E_CONSENT_LOG",
  "HOPLODEX_E2E_IDLE_MINUTE_SECONDS",
  "HOPLODEX_E2E_PDF_PREVIEW",
  "HOPLODEX_E2E_UPGRADE_PHASE_SECONDS",
];

/** The path of the log the latest start of the app writes (stdout and stderr). */
export function appLogPath(): string {
  return path.join(ensureLogDir(), `app-${currentRun().launches}.log`);
}

/**
 * Starts the check variant in its sandbox with `flatpak run` (support/app.ts's
 * launcher for HOPLODEX_E2E_FLATPAK=1): the run's home and buses, the
 * `HOPLODEX_E2E_*` variables of this process, the WebDriver port; its output
 * goes to flatpak/out/check/app-<n>.log.
 */
export function flatpakLauncher(): Launcher {
  return {
    start(port: number): ChildProcess {
      requireCheckEnvironment();
      const run = currentRun();
      const extraEnv: Record<string, string> = {};
      for (const name of [...APP_E2E_VARIABLES, ...APP_PASSTHROUGH]) {
        const value = process.env[name];
        if (value !== undefined) extraEnv[name] = value;
      }
      const { args, env } = flatpakRunArgs({
        home: run.home,
        documents: run.documents,
        port,
        extraEnv,
      });
      run.launches += 1;
      const fd = fs.openSync(appLogPath(), "w");
      const child = spawn("flatpak", args, {
        env: runProcessEnv(run.home, { ...env, DBUS_SESSION_BUS_ADDRESS: run.sessionBus.address }),
        stdio: ["ignore", fd, fd],
      });
      fs.closeSync(fd);
      return child;
    },
    afterKill: async () => killApp(),
  };
}

/**
 * The Documents folder as the app sees it: `xdg-user-dir DOCUMENTS` run inside
 * the sandbox under `home`, which reads the home's `user-dirs.dirs`.
 */
export function documentsDir(home: string = currentRun().home): string {
  requireCheckEnvironment();
  const result = spawnSync(
    "flatpak",
    ["run", "--command=xdg-user-dir", FLATPAK_APP_ID, "DOCUMENTS"],
    { env: runProcessEnv(home), encoding: "utf-8" },
  );
  const documents = result.stdout.trim();
  if (result.status !== 0 || !path.isAbsolute(documents)) {
    throw new Error(
      `xdg-user-dir DOCUMENTS in the sandbox failed (${result.status}): ${result.stderr.trim()}`,
    );
  }
  return documents;
}

/** The folder the opened documents' copies are made in while their database is
 * open: `opened-documents` under the app's cache folder (src-tauri/src/commands/documents.rs). */
export function copiesDir(): string {
  return path.join(appDataDir(currentRun().home), "cache", FLATPAK_APP_ID, "opened-documents");
}

// ---------------------------------------------------------------------------
// Looking at the sandbox from outside
// ---------------------------------------------------------------------------

/** The running instances of the app: `flatpak ps` lists a sandbox's process
 * (`child-pid`, in the container's namespace), which is the init of the sandbox's
 * own PID namespace; the app is its child. A stale row, whose process is gone,
 * is left out. */
function instances(): number[] {
  const result = spawnSync("flatpak", ["ps", "--columns=application,child-pid"], {
    encoding: "utf-8",
  });
  if (result.status !== 0) throw new Error(`flatpak ps failed: ${result.stderr.trim()}`);
  return result.stdout
    .split("\n")
    .map((line) => line.trim().split(/\s+/))
    .filter(([application, pid]) => application === FLATPAK_APP_ID && /^\d+$/.test(pid ?? ""))
    .map(([, pid]) => Number(pid))
    .filter((pid) => fs.existsSync(`/proc/${pid}`));
}

function procNumbers(): number[] {
  return fs
    .readdirSync("/proc")
    .filter((name) => /^\d+$/.test(name))
    .map(Number);
}

function readCommand(pid: number): string[] {
  try {
    return fs.readFileSync(`/proc/${pid}/cmdline`, "utf-8").split("\0").filter(Boolean);
  } catch {
    return [];
  }
}

function readExe(pid: number): string | undefined {
  try {
    return fs.readlinkSync(`/proc/${pid}/exe`);
  } catch {
    return undefined;
  }
}

/** A process's state letter and parent, or nothing once it is gone. */
function readStat(pid: number): { state: string; ppid: number } | undefined {
  try {
    const stat = fs.readFileSync(`/proc/${pid}/stat`, "utf-8");
    const fields = stat.slice(stat.lastIndexOf(")") + 2).split(" ");
    return { state: fields[0], ppid: Number(fields[1]) };
  } catch {
    return undefined;
  }
}

export interface SandboxProcess {
  pid: number;
  /** Its command line, as the sandbox's process was started (`hoplodex`, not a path). */
  command: string[];
  /** The file it runs, as that process sees it (`/app/bin/hoplodex` for the app). */
  exe: string | undefined;
}

/**
 * Every process in the app's sandbox (its PID namespace) as the container
 * sees it, from `flatpak ps` and /proc: the app and the helpers it started
 * (`--import-helper`, `--render-helper`), which the helpers' check reads
 * the command lines of. Empty when no sandbox is running.
 */
export function sandboxProcesses(): SandboxProcess[] {
  const found = new Map<number, SandboxProcess>();
  for (const init of instances()) {
    let namespace: string;
    try {
      namespace = fs.readlinkSync(`/proc/${init}/ns/pid`);
    } catch {
      continue;
    }
    for (const pid of procNumbers()) {
      try {
        if (fs.readlinkSync(`/proc/${pid}/ns/pid`) !== namespace) continue;
      } catch {
        continue;
      }
      if (readStat(pid)?.state === "Z") continue;
      found.set(pid, { pid, command: readCommand(pid), exe: readExe(pid) });
    }
  }
  return [...found.values()];
}

/** Ends the app's sandbox with `flatpak kill`, which does not count on the
 * app's own handlers (the helpers' check: they must die with it). Waits until
 * `flatpak ps` lists no instance of the app. */
export async function killApp(timeout = 10000): Promise<void> {
  spawnSync("flatpak", ["kill", FLATPAK_APP_ID], { stdio: "ignore" });
  const deadline = Date.now() + timeout;
  while (instances().length > 0) {
    if (Date.now() > deadline)
      throw new Error("the Flatpak sandbox was still running after `flatpak kill`");
    await sleep(50);
  }
}

/**
 * Sends SIGTERM to the app's own process, as systemd does to a Flatpak when the
 * session ends (011 research.md §12), and waits for it to exit. The process is
 * the sandbox init's child: `flatpak ps`'s `child-pid` is the init, which does
 * not pass the signal on.
 */
export async function terminateApp(timeout = 15000): Promise<void> {
  const [init] = instances();
  if (init === undefined) throw new Error("the app is not running in a Flatpak sandbox");
  const app = procNumbers().find(
    (pid) =>
      readStat(pid)?.ppid === init && path.basename(readCommand(pid)[0] ?? "") === "hoplodex",
  );
  if (app === undefined) throw new Error(`no hoplodex process under the sandbox's init (${init})`);
  process.kill(app, "SIGTERM");
  const deadline = Date.now() + timeout;
  for (;;) {
    const stat = readStat(app);
    if (!stat || stat.state === "Z") return;
    if (Date.now() > deadline)
      throw new Error(`the app did not exit within ${timeout} ms of SIGTERM`);
    await sleep(25);
  }
}

const LOCK_PROBE = `
import pathlib, sqlite3, sys
uri = pathlib.Path(sys.argv[1]).as_uri() + "?mode=ro"
try:
    connection = sqlite3.connect(uri, uri=True, timeout=0.2)
    connection.execute("PRAGMA schema_version").fetchall()
    print("unlocked")
except sqlite3.DatabaseError as error:
    message = str(error)
    if "database is locked" in message:
        print("locked")
    elif "not a database" in message:
        # An encrypted file read past the lock without its key: not locked.
        print("unlocked")
    else:
        print("error: " + message)
        sys.exit(1)
`;

/**
 * Whether the database file at `file` is locked, asked from outside the
 * sandbox: Python's `sqlite3` opens it read-only (`mode=ro`, so it can't
 * write) and reads `PRAGMA schema_version`; true when that gets "database is
 * locked". An encrypted file read without its key ("file is not a database")
 * got past the lock, so it is not locked. The exclusive lock holds across the
 * sandbox while the database is open (011 spec.md Edge Cases).
 */
export function lockProbe(file: string): boolean {
  const result = spawnSync("python3", ["-I", "-c", LOCK_PROBE, file], { encoding: "utf-8" });
  const answer = result.stdout.trim();
  if (answer === "locked") return true;
  if (answer === "unlocked") return false;
  throw new Error(`the lock probe failed on ${file}: ${answer || result.stderr.trim()}`);
}
