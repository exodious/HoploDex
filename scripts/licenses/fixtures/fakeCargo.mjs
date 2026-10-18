// A stand-in for the `cargo` runner that the notices generator takes as a
// parameter (010 tasks.md T011): it answers `cargo fetch`, `cargo tree` and
// `cargo metadata` from the output recorded in `cargo/`, so no test needs
// cargo, the network or the real crates. See README.md, "Recorded cargo output".

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));

/** The fake repository root: the good case (`project/`). Recorded output names it as `@ROOT@`. */
export const PROJECT = path.join(here, "project");

/** The three targets the audit checks (010 contracts/notices.md "Commands"). */
export const TRIPLES = [
  "x86_64-unknown-linux-gnu",
  "x86_64-pc-windows-msvc",
  "aarch64-apple-darwin",
];

function value(args, ...names) {
  for (let i = 0; i < args.length; i++) {
    for (const name of names) {
      if (args[i] === name) return args[i + 1];
      if (args[i].startsWith(`${name}=`)) return args[i].slice(name.length + 1);
    }
  }
  return undefined;
}

/**
 * The runner for one fixture: `cargo(args)` takes the arguments after `cargo`
 * (`["tree", "--locked", ..., "--target", "<triple>", ...]`) and returns what
 * cargo would print on stdout.
 *
 * - `fetch` prints nothing.
 * - `metadata` prints `cargo/metadata.json`.
 * - `tree` with `build` among its `-e`/`--edges` prints `cargo/build-<triple>.txt`
 *   (the packages with a build dependency on cc or cmake); any other `tree`
 *   prints `cargo/tree-<triple>.txt`.
 *
 * A `variant` (a folder under `cargo/variants/`) replaces just the files it
 * has. `@ROOT@` in the recorded output becomes `root`. Anything else throws, so
 * a command the fixtures can't answer fails loudly.
 */
export function fakeCargo({ variant, root = PROJECT } = {}) {
  const read = (file) => {
    const candidates = [
      ...(variant ? [path.join(here, "cargo", "variants", variant, file)] : []),
      path.join(here, "cargo", file),
    ];
    for (const candidate of candidates) {
      if (fs.existsSync(candidate)) {
        return fs.readFileSync(candidate, "utf8").replaceAll("@ROOT@", root.replaceAll("\\", "/"));
      }
    }
    throw new Error(`fakeCargo: no recorded output ${file}`);
  };
  return (args) => {
    const [subcommand] = args;
    if (subcommand === "fetch") return "";
    if (subcommand === "metadata") return read("metadata.json");
    if (subcommand === "tree") {
      const triple = value(args, "--target");
      if (!triple) throw new Error(`fakeCargo: cargo ${args.join(" ")} has no --target`);
      const edges = (value(args, "-e", "--edges") ?? "").split(",");
      return read(`${edges.includes("build") ? "build" : "tree"}-${triple}.txt`);
    }
    throw new Error(`fakeCargo: no recorded output for cargo ${args.join(" ")}`);
  };
}
