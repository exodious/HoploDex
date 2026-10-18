// @vitest-environment node
// The Vite plugin that writes the notices, checks the bundle and defines the
// program block (010 research.md §6, §10; 010 contracts/notices.md "The Vite
// plugin"; 010 FR-008, FR-011, FR-012): written first, red until vitePlugin.mjs
// exports hoplodexNotices. Each test runs Vite's own build() or createServer()
// on the tiny app in fixtures/vite-app (fixtures/README.md), whose packages are
// the fixture project's `modules/` (which stands for node_modules).
//
// The API these tests assume:
//
//   hoplodexNotices(options) -> { main, worker }   (two Vite plugins sharing
//        one collector: `main` goes in `plugins`, `worker` in `worker.plugins`)
//        options: what notices.mjs's buildNotices takes (root, lockFile,
//        modulesDir, licensesDir, licenseFile, cargo, git, now, env),
//        plus `target` (default: targetTriple()) and `withdrawn`
//        ([{ file, marker, reason }], default: policy.mjs's list; a test passes
//        the fixture's own entry). `modulesDir` also maps a module id to its
//        package: an id under it is that folder's package (scoped names have
//        two segments).
//        `main.config` is a plain function that returns `{ define }` and needs
//        no cargo, so Vitest, which uses vite.config.ts too, gets the
//        constants. `define.__HOPLODEX_PROGRAM__` is the program block as a
//        JSON string.
//        A failed build is `this.error` with every failure, one per line.
//        The development server serves GET /notices.json (application/json,
//        renderJson of the document with `development: true`), computed on
//        the first request and again after the lockfile changes, and logs each
//        failure with the server's logger `warn`.
import { copyFileSync, mkdtempSync, readFileSync, utimesSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { stripVTControlCharacters } from "node:util";
import { fileURLToPath } from "node:url";
import { build, createLogger, createServer, type InlineConfig } from "vite";
import { describe, expect, it } from "vitest";
import { fakeCargo, PROJECT, TRIPLES } from "./fixtures/fakeCargo.mjs";
import { buildNotices, main, programBlock, renderJson } from "./notices.mjs";
import { hoplodexNotices } from "./vitePlugin.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.join(here, "fixtures");
const appDir = path.join(fixtures, "vite-app");
const modules = path.join(PROJECT, "modules");
const [LINUX] = TRIPLES;
const MARKER = "fixture-withdrawn-marker-7Qz";
const WITHDRAWN = [
  { file: "vite-app/withdrawn.json", marker: MARKER, reason: "a fixture, 010 research.md §2" },
];

type Emitted = {
  fileName: string;
  type: "asset" | "chunk";
  source?: string | Uint8Array;
  code?: string;
};

function counting(cargo: (args: string[]) => string) {
  const calls: string[][] = [];
  const run = (args: string[]) => {
    calls.push(args);
    return cargo(args);
  };
  return Object.assign(run, { calls });
}

/** The plugin's options: the good fixtures for the Linux target. */
function options(over: Record<string, unknown> = {}) {
  return {
    target: LINUX,
    root: PROJECT,
    lockFile: path.join(PROJECT, "lock.json"),
    modulesDir: modules,
    licensesDir: path.join(PROJECT, "licenses"),
    licenseFile: path.join(PROJECT, "LICENSE"),
    cargo: fakeCargo(),
    git: () => "2026-10-18\n",
    now: new Date("2026-10-18T12:00:00Z"),
    env: { OPENSSL_DIR: path.join(fixtures, "openssl", "3.5.4") },
    withdrawn: WITHDRAWN,
    ...over,
  };
}

const alias = ["shipped-mit", "@scope/shipped-scoped", "dev-tool"].map((name) => ({
  find: new RegExp(`^${name}$`),
  replacement: path.join(modules, name, "index.js"),
}));

async function bundle(
  entry: string,
  over: Record<string, unknown> = {},
  warnings: string[] = [],
): Promise<Emitted[]> {
  const { main: mainPlugin, worker } = hoplodexNotices(options(over));
  const config: InlineConfig = {
    configFile: false,
    root: appDir,
    logLevel: "silent",
    plugins: [mainPlugin],
    worker: { format: "es", plugins: () => [worker] },
    resolve: { alias },
    build: {
      write: false,
      minify: false,
      reportCompressedSize: false,
      rollupOptions: {
        input: path.join(appDir, entry),
        onwarn: (warning) => void warnings.push(warning.message),
      },
    },
  };
  const result = await build(config);
  const outputs = Array.isArray(result) ? result : [result as { output: Emitted[] }];
  return outputs.flatMap((o) => (o as { output: Emitted[] }).output);
}

const asText = (item: Emitted | undefined) =>
  typeof item?.source === "string" ? item.source : Buffer.from(item?.source ?? []).toString("utf8");
const named = (output: Emitted[], fileName: string) => output.find((o) => o.fileName === fileName);
const tmp = () => mkdtempSync(path.join(os.tmpdir(), "hoplodex-vite-"));

describe("010 FR-008 - the build writes the notices", () => {
  it("emits notices.json and NOTICES.txt at the bundle's root", async () => {
    const output = await bundle("main.js");
    expect(named(output, "notices.json")?.type).toBe("asset");
    expect(named(output, "NOTICES.txt")?.type).toBe("asset");
    const doc = JSON.parse(asText(named(output, "notices.json")));
    expect(doc.format).toBe(1);
    expect(doc.target).toBe(LINUX);
  });

  it("emits what notices.mjs --out writes for the same inputs, byte for byte", async () => {
    const output = await bundle("main.js");
    const dir = tmp();
    const code = await main(["--out", dir, "--target", LINUX], {
      ...options(),
      env: {},
      hostTriple: LINUX,
      stdout: () => {},
      stderr: () => {},
    });
    expect(code).toBe(0);
    expect(asText(named(output, "notices.json"))).toBe(
      readFileSync(path.join(dir, "notices.json"), "utf8"),
    );
    expect(asText(named(output, "NOTICES.txt"))).toBe(
      readFileSync(path.join(dir, "NOTICES.txt"), "utf8"),
    );
  });

  it("lists the whole audit set, not only the packages the bundle uses", async () => {
    const doc = JSON.parse(asText(named(await bundle("main.js"), "notices.json")));
    const names = doc.components.map((c: { name: string }) => c.name);
    // the app imports shipped-mit and shipped-scoped only
    expect(names).toContain("with-notice-pkg");
    expect(names).toContain("choice-pkg");
    expect(names).not.toContain("dev-tool");
  });

  it("is byte-identical across two builds", async () => {
    const a = asText(named(await bundle("main.js"), "notices.json"));
    const b = asText(named(await bundle("main.js"), "notices.json"));
    expect(a).toBe(b);
  });

  it("passes the generator's warnings on to the build, and still builds", async () => {
    const warnings: string[] = [];
    const output = await bundle("main.js", {}, warnings);
    expect(named(output, "notices.json")).toBeDefined();
    // the good records list the WebView2 loader, which the Linux target doesn't ship
    expect(warnings.join("\n")).toContain("native code: crate webview2-com-sys");
  });

  it("writes notices for the plugin's target", async () => {
    const output = await bundle("main.js", { target: "x86_64-pc-windows-msvc" });
    const doc = JSON.parse(asText(named(output, "notices.json")));
    expect(doc.target).toBe("x86_64-pc-windows-msvc");
    expect(doc.components.some((c: { name: string }) => c.name === "OpenSSL")).toBe(true);
  });
});

describe("010 FR-012 - the bundle may hold only packages the audit counts as shipped", () => {
  const message = (pkg: string) =>
    `bundle: package ${pkg} is bundled into the app but isn't in the audit's shipped set (package-lock.json marks it dev). Check its dependency type.`;

  it("fails a dev package bundled into the main bundle, naming it", async () => {
    await expect(bundle("main-dev.js")).rejects.toThrow(message("dev-tool"));
  });

  it("fails a dev package only the strength worker's bundle holds", async () => {
    await expect(bundle("main-dev-worker.js")).rejects.toThrow(message("dev-tool"));
  });

  it("fails each package at most once, though both bundles hold it", async () => {
    let error: Error | undefined;
    try {
      await bundle("main-dev.js");
    } catch (e) {
      error = e as Error;
    }
    expect(error?.message.split(message("dev-tool")).length).toBe(2);
  });

  it("fails a bundled package the lockfile doesn't list at all, in the contract's second form", async () => {
    await expect(
      bundle("main.js", { lockFile: path.join(fixtures, "lock-unlisted", "lock.json") }),
    ).rejects.toThrow(
      "bundle: package shipped-mit is bundled into the app but isn't in the audit's shipped set (package-lock.json doesn't list it). Check its dependency type.",
    );
  });

  it("fails an unreadable lockfile, naming it", async () => {
    const lockFile = path.join(fixtures, "no-such-lock", "lock.json");
    await expect(bundle("main.js", { lockFile })).rejects.toThrow(
      `bundle: can't read ${lockFile} to check the bundled packages (`,
    );
  });

  it("passes a scoped package that is in the shipped set", async () => {
    const output = await bundle("main.js");
    const entry = output.find((o) => o.type === "chunk" && o.code?.includes("shipped-scoped"));
    expect(entry).toBeDefined();
  });
});

describe("010 FR-008 - a generator failure fails the build with the whole list", () => {
  it("says every failure, one per line", async () => {
    const licensesDir = path.join(fixtures, "records", "native-unlisted", "licenses");
    const failures = (
      buildNotices({ target: LINUX, ...options({ licensesDir }) }) as {
        failures: string[];
      }
    ).failures;
    expect(failures.length).toBeGreaterThan(1);
    let error: Error | undefined;
    try {
      await bundle("main.js", { licensesDir });
    } catch (e) {
      error = e as Error;
    }
    expect(error).toBeDefined();
    // Vite colours its errors on Windows even without a terminal (picocolors),
    // wrapping the first and last lines in escape codes.
    const lines = stripVTControlCharacters(error!.message).split("\n");
    for (const failure of failures) expect(lines, failure).toContain(failure);
  });

  it("emits nothing when it fails", async () => {
    // a failed build rejects, so there is no output to hold a notices file
    await expect(bundle("main.js", { cargo: fakeCargo({ variant: "gpl-crate" }) })).rejects.toThrow(
      "license: crate quux 2.0.0 is under GPL-2.0-only, which isn't allowed.",
    );
  });
});

describe("010 FR-014 - withdrawn data never reaches a chunk", () => {
  it("passes a build whose chunks hold no marker", async () => {
    const output = await bundle("main.js");
    // the worker's bundle is an asset of the main build, the entry a chunk
    const content = (o: Emitted) => (o.type === "chunk" ? (o.code ?? "") : asText(o));
    expect(output.filter((o) => o.fileName.endsWith(".js")).length).toBeGreaterThan(1);
    expect(output.some((o) => content(o).includes(MARKER))).toBe(false);
  });

  it("fails a marker in the main bundle, naming the file", async () => {
    await expect(bundle("main-withdrawn.js")).rejects.toThrow("vite-app/withdrawn.json");
  });

  it("fails a marker only in the worker's bundle, naming the file", async () => {
    await expect(bundle("main-withdrawn-worker.js")).rejects.toThrow("vite-app/withdrawn.json");
  });
});

describe("010 FR-002 - __HOPLODEX_PROGRAM__ holds the program block", () => {
  const expected = () => programBlock({ root: PROJECT, git: () => "2026-10-18\n" });

  it("is replaced in a build by the block notices.json carries", async () => {
    const output = await bundle("main.js");
    const entry = output.find((o) => o.type === "chunk" && o.code?.includes("console.log"));
    expect(entry?.code).not.toContain("__HOPLODEX_PROGRAM__");
    // every field of the block is in the entry chunk (the fixture's years are plain ASCII)
    for (const value of Object.values(expected())) expect(entry?.code).toContain(value);
    expect(JSON.parse(asText(named(output, "notices.json"))).program).toEqual(expected());
  });

  it("is defined by the config hook without running cargo, so Vitest gets it too", async () => {
    const cargo = counting(fakeCargo());
    const { main: plugin } = hoplodexNotices(options({ cargo }));
    for (const env of [
      { command: "build", mode: "production" },
      { command: "serve", mode: "development" },
      { command: "serve", mode: "test" },
    ]) {
      const result = await (
        plugin.config as (c: object, e: object) => Promise<{ define: Record<string, unknown> }>
      )({}, env);
      const value = result.define.__HOPLODEX_PROGRAM__;
      expect(typeof value === "string" ? JSON.parse(value) : value, env.mode).toEqual(expected());
    }
    expect(cargo.calls).toEqual([]);
  });

  it("is defined in the development server", async () => {
    const { server, stop } = await serve();
    try {
      const value = server.config.define?.__HOPLODEX_PROGRAM__;
      expect(typeof value === "string" ? JSON.parse(value) : value).toEqual(expected());
    } finally {
      await stop();
    }
  });
});

async function serve(over: Record<string, unknown> = {}) {
  const warnings: string[] = [];
  const logger = createLogger("silent");
  logger.warn = (msg) => void warnings.push(msg);
  const { main: plugin } = hoplodexNotices(options(over));
  const server = await createServer({
    configFile: false,
    root: appDir,
    customLogger: logger,
    plugins: [plugin],
    appType: "custom",
    resolve: { alias },
    optimizeDeps: { noDiscovery: true, include: [] },
    server: { host: "127.0.0.1", port: 0, strictPort: false, ws: false },
  });
  await server.listen();
  const base = server.resolvedUrls!.local[0];
  const get = async () => {
    const response = await fetch(new URL("notices.json", base));
    return { response, body: await response.text() };
  };
  return { server, warnings, get, stop: () => server.close() };
}

describe("010 FR-008 - the development server serves the notices", () => {
  it("serves GET /notices.json as JSON with development: true", async () => {
    const { get, stop } = await serve();
    try {
      const { response, body } = await get();
      expect(response.status).toBe(200);
      expect(response.headers.get("content-type")).toContain("application/json");
      const doc = JSON.parse(body);
      expect(doc.development).toBe(true);
      expect(doc.target).toBe(LINUX);
      const expected = (
        buildNotices({ target: LINUX, ...options(), development: true }) as {
          document: object;
        }
      ).document;
      expect(body).toBe(renderJson(expected));
    } finally {
      await stop();
    }
  });

  it("reports a generator failure as a warning and still serves what it found", async () => {
    const licensesDir = path.join(fixtures, "records", "native-unlisted", "licenses");
    const { get, warnings, stop } = await serve({ licensesDir });
    try {
      const { response, body } = await get();
      expect(response.status).toBe(200);
      const doc = JSON.parse(body);
      expect(doc.development).toBe(true);
      expect(doc.components.length).toBeGreaterThan(0);
      expect(warnings.join("\n")).toContain("native code: crate baz-sys 0.1.0");
    } finally {
      await stop();
    }
  });

  it("computes on the first request, then keeps the answer until the lockfile changes", async () => {
    const lockFile = path.join(tmp(), "lock.json");
    copyFileSync(path.join(PROJECT, "lock.json"), lockFile);
    const cargo = counting(fakeCargo());
    const { get, stop } = await serve({ lockFile, cargo });
    try {
      expect(cargo.calls).toEqual([]);
      const first = JSON.parse((await get()).body);
      const used = cargo.calls.length;
      expect(used).toBeGreaterThan(0);
      expect(first.components.some((c: { name: string }) => c.name === "shipped-mit")).toBe(true);

      await get();
      expect(cargo.calls).toHaveLength(used);

      // shipped-mit becomes development-only: it leaves the shipped set
      const lock = JSON.parse(readFileSync(lockFile, "utf8"));
      lock.packages["node_modules/shipped-mit"].dev = true;
      writeFileSync(lockFile, `${JSON.stringify(lock, null, 2)}\n`);
      const later = new Date(Date.now() + 60_000);
      utimesSync(lockFile, later, later);

      const deadline = Date.now() + 10_000;
      let after = first;
      while (Date.now() < deadline) {
        after = JSON.parse((await get()).body);
        if (!after.components.some((c: { name: string }) => c.name === "shipped-mit")) break;
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      expect(after.components.some((c: { name: string }) => c.name === "shipped-mit")).toBe(false);
      expect(after.components).toHaveLength(first.components.length - 1);
      expect(cargo.calls.length).toBeGreaterThan(used);
    } finally {
      await stop();
    }
  });
});
