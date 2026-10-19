// The Flatpak check's launcher (011 FR-021, 011 SC-007; 011 research.md §16):
// the argument builder is a pure function, so what `flatpak run` is given is
// pinned here without a Flatpak. `flatpakRunArgs` is e2e/support/flatpak.ts's
// (011 T030). The last test is 011 T032's: the default E2E run never includes
// the Flatpak journey.
import path from "node:path";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

type RunOptions = {
  home: string;
  documents: string;
  port: number;
  extraEnv?: Record<string, string>;
};
type Run = { args: string[]; env: Record<string, string> };

const APP_ID = "io.github.exodious.HoploDex";
const HOME = "/tmp/hoplodex-e2e-AbC123/home";
const DOCUMENTS = `${HOME}/Documents`;

type Flatpak = {
  FLATPAK_APP_ID?: string;
  flatpakRunArgs?: (options: RunOptions) => Run;
  requireCheckEnvironment?: () => { state: string };
};

// Loaded by a computed name, so that until e2e/support/flatpak.ts exists (011
// T030) the failure is one clear message in each test, not an unresolved
// import that fails the whole file, wdio.conf.ts's tests included.
let flatpak: Flatpak = {};
beforeAll(async () => {
  const file = "./flatpak";
  try {
    flatpak = (await import(/* @vite-ignore */ file)) as Flatpak;
  } catch (error) {
    flatpak = {};
    loadError = error;
  }
});
let loadError: unknown;

function build(options: Partial<RunOptions> = {}): Run {
  const run = flatpak.flatpakRunArgs;
  if (typeof run !== "function") {
    throw new Error(
      `e2e/support/flatpak.ts does not export flatpakRunArgs yet (011 T030): ${String(loadError ?? "")}`,
    );
  }
  return run({ home: HOME, documents: DOCUMENTS, port: 4555, ...options });
}

/** `--env=NAME=value` arguments as a map. */
function envArgs(args: string[]): Record<string, string> {
  return Object.fromEntries(
    args
      .filter((a) => a.startsWith("--env="))
      .map((a) => a.slice("--env=".length))
      .map((a) => [a.slice(0, a.indexOf("=")), a.slice(a.indexOf("=") + 1)]),
  );
}

describe("011 FR-021 - the argument list for `flatpak run` in the Flatpak check", () => {
  it("is `run`, then options, then the app id last, once", () => {
    const { args } = build();
    expect(args[0]).toBe("run");
    expect(args.at(-1)).toBe(APP_ID);
    expect(args.filter((a) => a === APP_ID)).toHaveLength(1);
  });

  it("011 FR-021: the app id is always io.github.exodious.HoploDex", () => {
    expect(flatpak.FLATPAK_APP_ID).toBe(APP_ID);
    for (const home of ["/a", "/b/c"]) expect(build({ home }).args.at(-1)).toBe(APP_ID);
  });

  it("011 SC-007: adds exactly --share=network beyond the shipped permissions", () => {
    const options = build().args.filter((a) => a.startsWith("-"));
    expect(options.filter((a) => !a.startsWith("--env="))).toEqual(["--share=network"]);
  });

  it("011 SC-007: never a filesystem, bus name, socket or device", () => {
    const { args } = build({ extraEnv: { HOPLODEX_E2E_KEYRING_FILE: "/x/keyring.json" } });
    for (const prefix of [
      "--filesystem",
      "--nofilesystem",
      "--talk-name",
      "--system-talk-name",
      "--own-name",
      "--socket",
      "--nosocket",
      "--device",
      "--nodevice",
      "--share=ipc",
      "--persist",
      "--command",
    ]) {
      expect(args.filter((a) => a.startsWith(prefix))).toEqual([]);
    }
  });

  it("011 FR-021: the e2e build's folders are the paths a shipped Flatpak uses", () => {
    const env = envArgs(build().args);
    const data = path.join(HOME, ".var", "app", APP_ID);
    expect(env.HOPLODEX_E2E_CONFIG_HOME).toBe(path.join(data, "config"));
    expect(env.HOPLODEX_E2E_CACHE_HOME).toBe(path.join(data, "cache"));
    expect(env.HOPLODEX_E2E_DOCUMENTS).toBe(DOCUMENTS);
    expect(env.HOPLODEX_E2E_HOME).toBe(HOME);
  });

  it("011 FR-021: the Documents folder is the one the caller read inside the sandbox", () => {
    const env = envArgs(build({ documents: "/some/where/Docs" }).args);
    expect(env.HOPLODEX_E2E_DOCUMENTS).toBe("/some/where/Docs");
  });

  it("gives the app the WebDriver port", () => {
    expect(envArgs(build({ port: 4701 }).args).TAURI_WEBDRIVER_PORT).toBe("4701");
  });

  it("passes the caller's extra variables to the app and lets none override the four folders", () => {
    const env = envArgs(
      build({
        extraEnv: {
          HOPLODEX_E2E_KEYRING_FILE: "/x/keyring.json",
          HOPLODEX_E2E_CONFIG_HOME: "/elsewhere",
        },
      }).args,
    );
    expect(env.HOPLODEX_E2E_KEYRING_FILE).toBe("/x/keyring.json");
    expect(env.HOPLODEX_E2E_CONFIG_HOME).toBe(path.join(HOME, ".var", "app", APP_ID, "config"));
  });

  it("011 SC-007: HOME for `flatpak run` is the run's throwaway home, never the real one", () => {
    const { env } = build();
    expect(env.HOME).toBe(HOME);
    expect(env.HOME).not.toBe(process.env.HOME);
  });

  it("011 SC-007: refuses a relative or empty home", () => {
    build();
    expect(() => build({ home: "" })).toThrow();
    expect(() => build({ home: "home" })).toThrow();
  });
});

describe("011 FR-021 - the default E2E run never includes the Flatpak journey", () => {
  const saved = process.env.HOPLODEX_E2E_FLATPAK;
  afterEach(() => {
    if (saved === undefined) delete process.env.HOPLODEX_E2E_FLATPAK;
    else process.env.HOPLODEX_E2E_FLATPAK = saved;
    vi.resetModules();
  });

  async function config(flatpakRun: boolean) {
    if (flatpakRun) process.env.HOPLODEX_E2E_FLATPAK = "1";
    else delete process.env.HOPLODEX_E2E_FLATPAK;
    vi.resetModules();
    return (await import("../wdio.conf")).config;
  }

  const named = (list: unknown) =>
    (Array.isArray(list) ? list.flat() : []).map(String).filter((s) => s.includes("flatpak.e2e"));

  // The first test to import e2e/wdio.conf.ts pays for its cold import, which once passed Vitest's 5 s (011 T113).
  it(
    "011 FR-021: the default list names no flatpak.e2e.ts and excludes it from the glob",
    { timeout: 60_000 },
    async () => {
      const { specs, exclude } = await config(false);
      expect(named(specs)).toEqual([]);
      expect(named(exclude)).toEqual(["./specs/flatpak.e2e.ts"]);
    },
  );

  it("011 FR-021: HOPLODEX_E2E_FLATPAK=1 runs only flatpak.e2e.ts, with one worker", async () => {
    const { specs, maxInstances } = await config(true);
    expect(specs).toEqual(["./specs/flatpak.e2e.ts"]);
    expect(maxInstances).toBe(1);
  });
});

// 011 T105: every `flatpak` command the check's support code runs comes after this, so
// without the variables check.sh (and env.sh's guard) sets, nothing may run.
describe("011 SC-007 - requireCheckEnvironment refuses outside check.sh", () => {
  const NAMES = [
    "FLATPAK_USER_DIR",
    "HOPLODEX_FLATPAK_STATE",
    "DBUS_SYSTEM_BUS_ADDRESS",
    "HOPLODEX_FLATPAK_CHECK",
  ];
  const full: Record<string, string> = {
    FLATPAK_USER_DIR: "/tmp/hoplodex-throwaway",
    HOPLODEX_FLATPAK_STATE: "/tmp/hoplodex-state",
    DBUS_SYSTEM_BUS_ADDRESS: "unix:path=/tmp/bus",
    HOPLODEX_FLATPAK_CHECK: "1",
  };
  afterEach(() => vi.unstubAllEnvs());

  function require(env: Record<string, string>) {
    for (const name of NAMES) vi.stubEnv(name, env[name] ?? "");
    const fn = flatpak.requireCheckEnvironment;
    if (typeof fn !== "function") throw new Error("requireCheckEnvironment is not exported");
    return fn();
  }

  it("returns the state folder when check.sh set all four", () => {
    expect(require(full)).toEqual({ state: "/tmp/hoplodex-state" });
  });

  it.each(NAMES)("refuses when %s alone is missing, naming it", (missing) => {
    const env = { ...full };
    delete env[missing];
    expect(() => require(env)).toThrow(new RegExp(`${missing} is not set`));
  });

  it("refuses with none set, naming every one and the command to use", () => {
    expect(() => require({})).toThrow(/test:flatpak/);
    try {
      require({});
    } catch (error) {
      for (const name of NAMES) expect(String(error)).toContain(name);
      expect(String(error)).toContain("are not set");
    }
  });
});
