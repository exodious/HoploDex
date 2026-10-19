// @vitest-environment node
// The host guard (011 US5-3, 011 SC-007; 011 research.md §15, §16): the Flatpak
// scripts run only in the dev container's `--flatpak` mode, or against a
// throwaway installation the caller names on a host. The decision is a pure
// function of the environment, hostname, arguments and a file-system reader, so
// no test here touches a real Flatpak installation.
import { describe, expect, it } from "vitest";
import { CONTAINER_HOSTNAME, CONTAINER_MARKER_FILE, decide, MARKER_NAME } from "./guard.mjs";

const HOME = "/home/someone";
const OWN_INSTALLATION = `${HOME}/.local/share/flatpak`;
const HOST_FLAG = "--i-know-this-is-a-host";

type Entry = { kind: "directory"; entries: string[] } | { kind: "other" };

/** An in-memory file system: `files` exist, `dirs` map a folder to what it holds, `links` map a path to its target. */
function fakeFs(
  opts: { files?: string[]; dirs?: Record<string, string[]>; links?: Record<string, string> } = {},
) {
  const files = new Set(opts.files ?? []);
  const dirs = opts.dirs ?? {};
  const links = opts.links ?? {};
  const resolve = (p: string) => links[p] ?? p;
  const entry = (p: string): Entry | undefined => {
    const r = resolve(p);
    if (r in dirs) return { kind: "directory", entries: dirs[r] };
    if (files.has(r)) return { kind: "other" };
    return undefined;
  };
  return {
    exists: (p: string) => files.has(resolve(p)) || resolve(p) in dirs,
    stat: (p: string) => entry(p)?.kind ?? "missing",
    readdir: (p: string) => {
      const e = entry(p);
      return e && e.kind === "directory" ? e.entries : [];
    },
    realpath: resolve,
  };
}

const containerEnv = { HOPLODEX_DEV_CONTAINER: "1", HOPLODEX_FLATPAK_STATE: "/flatpak-state" };
const containerFs = () => fakeFs({ files: [CONTAINER_MARKER_FILE] });

function onHost(env: Record<string, string>, argv: string[], fs = fakeFs()) {
  return decide({ env, hostname: "nous", argv, home: HOME, fs });
}

describe("011 US5-3, 011 SC-007 - the host guard", () => {
  describe("inside the dev container", () => {
    it("allows, with FLATPAK_USER_DIR defaulting to the container user's installation", () => {
      const r = decide({
        env: containerEnv,
        hostname: CONTAINER_HOSTNAME,
        argv: [],
        home: "/home/dev",
        fs: containerFs(),
      });
      expect(r).toEqual({
        ok: true,
        flatpakUserDir: "/home/dev/.local/share/flatpak",
        writeMarker: false,
      });
    });

    it("keeps a FLATPAK_USER_DIR the caller set", () => {
      const r = decide({
        env: { ...containerEnv, FLATPAK_USER_DIR: "/home/dev/other" },
        hostname: CONTAINER_HOSTNAME,
        argv: [],
        home: "/home/dev",
        fs: containerFs(),
      });
      expect(r).toMatchObject({ ok: true, flatpakUserDir: "/home/dev/other" });
    });

    it("refuses without --flatpak (no HOPLODEX_FLATPAK_STATE), naming scripts/dev-container.sh --flatpak", () => {
      const r = decide({
        env: { HOPLODEX_DEV_CONTAINER: "1" },
        hostname: CONTAINER_HOSTNAME,
        argv: [],
        home: "/home/dev",
        fs: containerFs(),
      });
      expect(r.ok).toBe(false);
      if (!r.ok) expect(r.message).toContain("scripts/dev-container.sh --flatpak");
    });

    it("refuses an empty HOPLODEX_FLATPAK_STATE", () => {
      const r = decide({
        env: { ...containerEnv, HOPLODEX_FLATPAK_STATE: "" },
        hostname: CONTAINER_HOSTNAME,
        argv: [],
        home: "/home/dev",
        fs: containerFs(),
      });
      expect(r.ok).toBe(false);
    });
  });

  describe("the container's markers", () => {
    // A host can share any one of the three, so each alone is not enough.
    it("the environment variable alone is not enough", () => {
      const r = onHost(containerEnv, []);
      expect(r.ok).toBe(false);
    });

    it("the marker file alone is not enough", () => {
      const r = onHost({}, [], fakeFs({ files: [CONTAINER_MARKER_FILE] }));
      expect(r.ok).toBe(false);
    });

    it("the hostname alone is not enough", () => {
      const r = decide({
        env: {},
        hostname: CONTAINER_HOSTNAME,
        argv: [],
        home: HOME,
        fs: fakeFs(),
      });
      expect(r.ok).toBe(false);
    });

    it("any two of the three are not enough, even with --flatpak's state set", () => {
      const pairs: [Record<string, string>, string, ReturnType<typeof fakeFs>][] = [
        [containerEnv, CONTAINER_HOSTNAME, fakeFs()],
        [containerEnv, "nous", containerFs()],
        [{ HOPLODEX_FLATPAK_STATE: "/s" }, CONTAINER_HOSTNAME, containerFs()],
      ];
      for (const [env, hostname, fs] of pairs) {
        const r = decide({ env, hostname, argv: [], home: HOME, fs });
        expect(r.ok).toBe(false);
      }
    });
  });

  describe("on a host", () => {
    it("refuses with nothing set, naming the dev container and FLATPAK_USER_DIR", () => {
      const r = onHost({}, []);
      expect(r.ok).toBe(false);
      if (!r.ok) {
        expect(r.message).toContain("dev container");
        expect(r.message).toContain("FLATPAK_USER_DIR");
        expect(r.message).toContain(HOST_FLAG);
      }
    });

    it("refuses with only FLATPAK_USER_DIR set", () => {
      const r = onHost({ FLATPAK_USER_DIR: "/tmp/throwaway" }, []);
      expect(r.ok).toBe(false);
    });

    it("refuses with only the flag passed", () => {
      const r = onHost({}, [HOST_FLAG]);
      expect(r.ok).toBe(false);
    });

    it("allows both with an empty folder, and asks for the marker to be written", () => {
      const r = onHost(
        { FLATPAK_USER_DIR: "/tmp/throwaway" },
        [HOST_FLAG],
        fakeFs({ dirs: { "/tmp/throwaway": [] } }),
      );
      expect(r).toEqual({ ok: true, flatpakUserDir: "/tmp/throwaway", writeMarker: true });
    });

    it("allows both with a folder that does not exist yet", () => {
      const r = onHost({ FLATPAK_USER_DIR: "/tmp/new" }, [HOST_FLAG]);
      expect(r).toMatchObject({ ok: true, flatpakUserDir: "/tmp/new" });
    });

    it("allows both with a folder holding the guard's own marker", () => {
      const r = onHost(
        { FLATPAK_USER_DIR: "/tmp/used" },
        [HOST_FLAG],
        fakeFs({ dirs: { "/tmp/used": [MARKER_NAME, "repo", "app"] } }),
      );
      expect(r).toMatchObject({ ok: true, flatpakUserDir: "/tmp/used" });
    });

    it("accepts the flag among other arguments", () => {
      const r = onHost(
        { FLATPAK_USER_DIR: "/tmp/t" },
        ["--fresh-install", HOST_FLAG],
        fakeFs({ dirs: { "/tmp/t": [] } }),
      );
      expect(r.ok).toBe(true);
    });

    it("refuses a folder that is not empty and unmarked", () => {
      const r = onHost(
        { FLATPAK_USER_DIR: "/tmp/stuff" },
        [HOST_FLAG],
        fakeFs({ dirs: { "/tmp/stuff": ["repo", "app"] } }),
      );
      expect(r.ok).toBe(false);
    });

    it("refuses something that is not a folder", () => {
      const r = onHost(
        { FLATPAK_USER_DIR: "/tmp/file" },
        [HOST_FLAG],
        fakeFs({ files: ["/tmp/file"] }),
      );
      expect(r.ok).toBe(false);
    });

    it("refuses a relative path", () => {
      const r = onHost({ FLATPAK_USER_DIR: "throwaway" }, [HOST_FLAG]);
      expect(r.ok).toBe(false);
    });

    it.each([OWN_INSTALLATION, "/var/lib/flatpak"])(
      "refuses %s, even when empty or marked",
      (dir) => {
        for (const entries of [[], [MARKER_NAME]]) {
          const r = onHost(
            { FLATPAK_USER_DIR: dir },
            [HOST_FLAG],
            fakeFs({ dirs: { [dir]: entries } }),
          );
          expect(r.ok).toBe(false);
        }
      },
    );

    it("refuses a folder inside the host's own installation", () => {
      const dir = `${OWN_INSTALLATION}/sub`;
      const r = onHost({ FLATPAK_USER_DIR: dir }, [HOST_FLAG], fakeFs({ dirs: { [dir]: [] } }));
      expect(r.ok).toBe(false);
    });

    it("refuses a path that resolves to the host's own installation", () => {
      const r = onHost(
        { FLATPAK_USER_DIR: "/tmp/innocent" },
        [HOST_FLAG],
        fakeFs({ dirs: { [OWN_INSTALLATION]: [] }, links: { "/tmp/innocent": OWN_INSTALLATION } }),
      );
      expect(r.ok).toBe(false);
    });

    it("refuses a path with .. that reaches /var/lib/flatpak", () => {
      const r = onHost(
        { FLATPAK_USER_DIR: "/tmp/../var/lib/flatpak" },
        [HOST_FLAG],
        fakeFs({ dirs: { "/var/lib/flatpak": [] } }),
      );
      expect(r.ok).toBe(false);
    });

    it("refuses the installation XDG_DATA_HOME names", () => {
      const dir = "/data/share/flatpak";
      const r = onHost(
        { FLATPAK_USER_DIR: dir, XDG_DATA_HOME: "/data/share" },
        [HOST_FLAG],
        fakeFs({ dirs: { [dir]: [] } }),
      );
      expect(r.ok).toBe(false);
    });

    it("refuses a folder that holds the host's installation", () => {
      const r = onHost(
        { FLATPAK_USER_DIR: `${HOME}/.local/share` },
        [HOST_FLAG],
        fakeFs({ dirs: { [`${HOME}/.local/share`]: [] } }),
      );
      expect(r.ok).toBe(false);
    });
  });
});
