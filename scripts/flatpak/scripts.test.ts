// @vitest-environment node
// The Flatpak scripts reach the host guard before any `flatpak`, `flatpak-builder` or
// `dbus-daemon` call (011 US5-3, 011 SC-007; 011 T105), and on a host build.sh gets its
// state folder from the guard (011 T104). The scripts run for real under `bash`, with
// the container's markers absent: the guard needs HOPLODEX_DEV_CONTAINER=1 together
// with the container's hostname and marker file, and the environment given here has
// no such variable, so it sees a host even when the tests run inside the container
// (no bypass in the scripts or the guard). Stubs for the three commands come first on
// PATH and record every call to a file, which the tests then expect to be empty.
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const HOST_FLAG = "--i-know-this-is-a-host";
const STUBBED = ["flatpak", "flatpak-builder", "dbus-daemon"];

// The scripts are bash for Linux; the Windows run of the suite has no Flatpak.
const describeScripts = process.platform === "win32" ? describe.skip : describe;

let root: string;
let stubs: string;
let calls: string;
let home: string;

/** A new empty folder under the test's own root. */
function folder(name: string): string {
  const dir = fs.mkdtempSync(path.join(root, `${name}-`));
  return dir;
}

beforeAll(() => {
  root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-guard-scripts-")));
  stubs = path.join(root, "stubs");
  fs.mkdirSync(stubs);
  calls = path.join(root, "calls.log");
  fs.writeFileSync(calls, "");
  home = path.join(root, "home");
  fs.mkdirSync(home);
  for (const name of STUBBED) {
    // dbus-daemon is asked for an address and a process id; the id is one no process has.
    const answer = name === "dbus-daemon" ? 'echo "unix:path=/nonexistent"; echo 2147483646' : "";
    fs.writeFileSync(
      path.join(stubs, name),
      `#!/bin/sh\necho "${name} $*" >> "${calls}"\n${answer}\n`,
      { mode: 0o755 },
    );
  }
});

afterAll(() => {
  fs.rmSync(root, { recursive: true, force: true });
});

/** Runs a script under bash with only what the test names: no container variable, no FLATPAK_*. */
function run(args: string[], env: Record<string, string> = {}) {
  fs.writeFileSync(calls, "");
  const result = spawnSync("/bin/bash", args, {
    encoding: "utf-8",
    cwd: path.join(HERE, "../.."),
    timeout: 60_000,
    env: {
      PATH: [stubs, path.dirname(process.execPath), "/usr/bin", "/bin"].join(path.delimiter),
      HOME: home,
      TMPDIR: root,
      ...env,
    },
  });
  return {
    status: result.status,
    stdout: result.stdout,
    stderr: result.stderr,
    calls: fs.readFileSync(calls, "utf-8"),
  };
}

const script = (name: string) => path.join(HERE, name);

describeScripts(
  "011 US5-3, 011 SC-007 - the scripts reach the host guard before any Flatpak tool",
  () => {
    it.each(["build.sh", "check.sh", "sources.sh"])(
      "%s refuses on a host with nothing set, and calls no stub",
      (name) => {
        const r = run([script(name)]);
        expect(r.status).not.toBe(0);
        expect(r.stderr).toContain("dev container");
        expect(r.calls).toBe("");
      },
    );

    it.each(["build.sh", "check.sh", "sources.sh"])(
      "%s refuses on a host with only the flag, and calls no stub",
      (name) => {
        const r = run([script(name), HOST_FLAG]);
        expect(r.status).not.toBe(0);
        expect(r.calls).toBe("");
      },
    );

    it.each(["build.sh", "sources.sh"])(
      "%s refuses a FLATPAK_USER_DIR that holds things it did not make, and calls no stub",
      (name) => {
        const dir = folder("holds");
        fs.writeFileSync(path.join(dir, "somebody-elses-file"), "x");
        const r = run([script(name), HOST_FLAG], { FLATPAK_USER_DIR: dir });
        expect(r.status).not.toBe(0);
        expect(r.stderr).toContain("not empty");
        expect(r.calls).toBe("");
      },
    );

    it("build.sh refuses the host's own installation, and calls no stub", () => {
      const own = path.join(home, ".local/share/flatpak");
      fs.mkdirSync(own, { recursive: true });
      const r = run([script("build.sh"), HOST_FLAG], { FLATPAK_USER_DIR: own });
      expect(r.status).not.toBe(0);
      expect(r.stderr).toContain("real Flatpak installation");
      expect(r.calls).toBe("");
    });

    it("011 T104: check.sh refuses on a host even with a throwaway installation and the flag, saying why", () => {
      const dir = folder("throwaway");
      const r = run([script("check.sh"), HOST_FLAG], { FLATPAK_USER_DIR: dir });
      expect(r.status).not.toBe(0);
      expect(r.stderr).toContain("only in the dev container");
      expect(r.stderr).toContain("Xvfb");
      expect(r.calls).toBe("");
      expect(fs.readdirSync(dir)).toEqual([]);
    });

    it("011 T104: check.sh --fresh-install refuses on a host the same way", () => {
      const dir = folder("fresh");
      const r = run([script("check.sh"), "--fresh-install", HOST_FLAG], { FLATPAK_USER_DIR: dir });
      expect(r.status).not.toBe(0);
      expect(r.stderr).toContain("only in the dev container");
      expect(r.calls).toBe("");
    });
  },
);

describeScripts("011 T104 - env.sh on a host without podman", () => {
  /** Sources env.sh as build.sh does, then prints what it exported. */
  const source = (env: Record<string, string>) =>
    run(
      [
        "-c",
        `source "${script("env.sh")}" ${HOST_FLAG} && echo "USER_DIR=$FLATPAK_USER_DIR" && echo "STATE=$HOPLODEX_FLATPAK_STATE"`,
      ],
      env,
    );

  it("defaults HOPLODEX_FLATPAK_STATE to a folder beside the throwaway installation", () => {
    const dir = path.join(folder("host"), "flatpak");
    const r = source({ FLATPAK_USER_DIR: dir });
    expect(r.stderr).toBe("");
    expect(r.status).toBe(0);
    expect(r.stdout).toContain(`USER_DIR=${dir}\n`);
    expect(r.stdout).toContain(`STATE=${dir}-state\n`);
    // The marker says the guard accepted it; only the remote and the private bus were touched.
    expect(fs.existsSync(path.join(dir, ".hoplodex-throwaway-flatpak"))).toBe(true);
    const lines = r.calls.split("\n").filter(Boolean);
    expect(lines).toHaveLength(2);
    expect(lines[0]).toMatch(/^dbus-daemon /);
    expect(lines[1]).toMatch(/^flatpak remote-add /);
  });

  it("keeps a HOPLODEX_FLATPAK_STATE the caller named", () => {
    const dir = path.join(folder("host"), "flatpak");
    const state = path.join(folder("state"), "mine");
    const r = source({ FLATPAK_USER_DIR: dir, HOPLODEX_FLATPAK_STATE: state });
    expect(r.status).toBe(0);
    expect(r.stdout).toContain(`STATE=${state}\n`);
  });
});
