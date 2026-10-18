// hoplodexNotices(): the Vite plugin that defines the build constants, checks
// the bundled packages, and emits notices.json and NOTICES.txt (010 research.md
// §6, §10; 010 contracts/notices.md, "The Vite plugin").
//
// It returns two plugins that share one collector: `main` goes in `plugins`,
// `worker` in `worker.plugins`, so the packages of the main bundle and of the
// strength worker's are both known when the main build writes its bundle.

import { readFileSync, statSync } from "node:fs";
import path from "node:path";
import { buildNotices, programBlock, renderJson, renderText, resolveInputs } from "./notices.mjs";
import { WITHDRAWN } from "./policy.mjs";
import { npmPackages, targetTriple } from "./shipped.mjs";

const NAME = "hoplodex-notices";

/** Forward slashes, and lower case where the file system ignores case. */
const normalize = (file) => {
  const slashed = file.replaceAll("\\", "/");
  return process.platform === "win32" ? slashed.toLowerCase() : slashed;
};

/**
 * The npm package a module id belongs to, or undefined for a module outside
 * `modulesDir`. The id may carry a query (`?worker`) or a plugin's prefix
 * (`\0commonjs-proxy:`); a package nested in another's `node_modules` is its
 * own; a scoped name has two segments.
 */
export function packageOf(id, modulesDir) {
  const folder = `${normalize(modulesDir).replace(/\/+$/, "")}/`;
  const clean = normalize(id.replace(/^\0/, "").replace(/\?.*$/, ""));
  const at = clean.indexOf(folder);
  if (at === -1) return undefined;
  // The same case as the id, for the name
  let rest = id
    .replace(/^\0/, "")
    .replace(/\?.*$/, "")
    .replaceAll("\\", "/")
    .slice(at + folder.length);
  const nested = rest.lastIndexOf("/node_modules/");
  if (nested !== -1) rest = rest.slice(nested + "/node_modules/".length);
  const segments = rest.split("/");
  const name = segments[0].startsWith("@") ? segments.slice(0, 2).join("/") : segments[0];
  return name === "" ? undefined : name;
}

/** What a failed build says: a header line, then every failure on a line of its own. */
const failureMessage = (failures) =>
  `notices: the build can't continue (${failures.length} ${failures.length === 1 ? "failure" : "failures"}):\n${failures.join("\n")}`;

/** The text of an emitted file, when it can hold a withdrawn data file's marker. */
function textOf(item) {
  if (item.type === "chunk") return item.code;
  if (!/\.(?:m?js|cjs|json|html|css|txt)$/i.test(item.fileName)) return undefined;
  return typeof item.source === "string" ? item.source : Buffer.from(item.source).toString("utf8");
}

/**
 * `options` is what `buildNotices` takes (root, lockFile, modulesDir,
 * licensesDir, licenseFile, cargo, git, now, env), plus `target`
 * (default: `targetTriple()`) and `withdrawn` (default: policy.mjs's list).
 */
export function hoplodexNotices(options = {}) {
  const { target, withdrawn = WITHDRAWN, ...rest } = options;
  const inputs = resolveInputs(rest);
  const bundled = new Set();
  const withdrawnFound = new Set();

  const generate = (extra) =>
    buildNotices({
      ...rest,
      ...extra,
      target: target ?? targetTriple({ argv: [], env: inputs.env }),
    });

  const collect = {
    buildEnd() {
      for (const id of this.getModuleIds()) {
        const name = packageOf(id, inputs.modulesDir);
        if (name !== undefined) bundled.add(name);
      }
    },
  };
  const watchWithdrawn = (bundle) => {
    for (const item of Object.values(bundle)) {
      const text = textOf(item);
      if (text === undefined) continue;
      for (const entry of withdrawn) {
        if (text.includes(entry.marker)) withdrawnFound.add(entry);
      }
    }
  };

  /** The bundled-package check (010 FR-012): every package in either bundle must ship. */
  const bundleFailures = () => {
    let lock;
    try {
      lock = JSON.parse(readFileSync(inputs.lockFile, "utf8"));
    } catch (error) {
      return [
        `bundle: can't read ${inputs.lockFile} to check the bundled packages (${error.message}).`,
      ];
    }
    const packages = npmPackages(lock);
    const shipped = new Set(packages.filter((p) => !p.development).map((p) => p.name));
    const known = new Set(packages.map((p) => p.name));
    return [...bundled]
      .filter((name) => !shipped.has(name))
      .sort()
      .map((name) =>
        known.has(name)
          ? `bundle: package ${name} is bundled into the app but isn't in the audit's shipped set (package-lock.json marks it dev). Check its dependency type.`
          : `bundle: package ${name} is bundled into the app but isn't in the audit's shipped set (package-lock.json doesn't list it). Check its dependency type.`,
      );
  };

  // The development server's answer, kept until the lockfile changes.
  let served;

  const main = {
    name: NAME,

    // The constants only: no `cargo`, so Vitest, which uses this config, gets them too.
    config() {
      const program = programBlock({ root: inputs.root, git: inputs.git, now: inputs.now });
      return { define: { __HOPLODEX_PROGRAM__: JSON.stringify(program) } };
    },

    ...collect,

    generateBundle(_outputOptions, bundle) {
      watchWithdrawn(bundle);
      const failures = bundleFailures();
      for (const entry of withdrawn) {
        if (withdrawnFound.has(entry)) {
          failures.push(`withdrawn: ${entry.file} is in the build (${entry.reason}).`);
        }
      }
      const built = generate({});
      failures.push(...built.failures);
      for (const warning of built.warnings) this.warn(warning);
      if (failures.length > 0) this.error(failureMessage(failures));
      this.emitFile({
        type: "asset",
        fileName: "notices.json",
        source: renderJson(built.document),
      });
      this.emitFile({ type: "asset", fileName: "NOTICES.txt", source: renderText(built.document) });
    },

    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        if ((request.url ?? "").split("?")[0] !== "/notices.json") return next();
        let stamp;
        try {
          const stat = statSync(inputs.lockFile);
          stamp = `${stat.mtimeMs}:${stat.size}`;
        } catch {
          stamp = "no lockfile";
        }
        if (served === undefined || served.stamp !== stamp) {
          const built = generate({ development: true });
          for (const failure of built.failures) server.config.logger.warn(failure);
          served = { stamp, body: renderJson(built.document) };
        }
        response.statusCode = 200;
        response.setHeader("Content-Type", "application/json; charset=utf-8");
        response.setHeader("Cache-Control", "no-store");
        response.end(served.body);
      });
    },
  };

  // The strength worker's bundle is its own build: its modules count, and its
  // chunks are searched for withdrawn data before the main build sees them.
  const worker = {
    name: `${NAME}-worker`,
    ...collect,
    generateBundle(_outputOptions, bundle) {
      watchWithdrawn(bundle);
    },
  };

  return { main, worker };
}
