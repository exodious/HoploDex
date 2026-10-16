// Spec citations name their spec (#121). Spec Kit numbers FR, SC, user
// story and task IDs, and each feature's research.md, plan.md and contracts,
// within each spec, so a bare `FR-040` or `research.md §5` could mean any of
// them. Outside a spec's own folder a citation names its spec first:
// `003 FR-040`, `006 SC-002`, `001 US2`, `007 US3-2` (story 3, acceptance
// scenario 2), `008 T050`, `007 research.md §5`, `006 contracts/ui-accessories.md §3`.
// One spec number covers the list that follows it (`006 FR-001, FR-002 and
// SC-003`, `003 research.md §1, §11`), and a path into `specs/NNN-name/`
// names its spec by itself. "User Story 2" and "Scenario 3" are always
// written as `US2` and `US1-3` with their spec.
//
// This also checks that each cited ID, story, scenario, task, document and
// section exists in the spec it names, and that every E2E spec file's
// `describe()` titles name a story by its spec's own heading (DEVELOPMENT.md,
// "E2E files").
//
// `node scripts/citations.mjs [file …]` lists what's wrong, in every checked
// file or in the ones named; `scripts/citations.test.ts` runs it in Vitest.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** Where citations are checked (#121): the code, the tests and the tooling. */
export const CHECKED_DIRS = ["src", "src-tauri", "e2e", "scripts", "tools"];

/** Files that quote bare citations on purpose: this checker and its tests. */
const EXEMPT = new Set(["scripts/citations.mjs", "scripts/citations.test.ts"]);

/** Vendored code, which cites other projects' documents, not the specs. */
const EXEMPT_DIRS = ["src-tauri/vendor/"];

/** A standard whose sections a comment may cite: `W3C WebDriver §13.2.2`. */
const OTHER_DOCUMENT = /\b(?:W3C|WebDriver|RFC ?\d+)\b[^.;()]{0,40}$/;

/** E2E spec files that test the harness, not a story. */
const E2E_HARNESS = new Set(["e2e/specs/webdriver-plugin.e2e.ts"]);

const ID = String.raw`(?:FR|SC)-\d+[a-z]?|US\d+(?:-\d+[a-z]?)?|T\d{3}[a-z]?`;
const SECTION = String.raw`§ ?\d+[a-z]?(?:\.\d+)*`;
/** Words that cite a story or scenario without its `US` form, or a spec's section without its spec. */
const PROSE = String.raw`\bUser Stor(?:y|ies) \d+|\bScenarios? \d+|\bspec(?:'s)? (?:Edge Cases|Assumptions|Clarifications)\b`;
/** What may join a citation to the one before it, which then names the spec. */
const JOINER = /^(?:,? (?:and|or|to) |, |,? and |,? or | ?[/–-] ?| |'s |: )$/;

/** Every spec under `specs/`: its number, folder, documents and what they define. */
export function loadSpecs(root = ROOT) {
  const specs = new Map();
  for (const dir of fs.readdirSync(path.join(root, "specs")).sort()) {
    const match = /^(\d{3})-/.exec(dir);
    if (!match) continue;
    const base = path.join(root, "specs", dir);
    const docs = new Map();
    for (const sub of ["", "contracts", "checklists"]) {
      const folder = path.join(base, sub);
      if (!fs.existsSync(folder)) continue;
      for (const name of fs.readdirSync(folder)) {
        if (name.endsWith(".md")) {
          docs.set(sub ? `${sub}/${name}` : name, fs.readFileSync(path.join(folder, name), "utf8"));
        }
      }
    }
    const spec = docs.get("spec.md") ?? "";
    const stories = new Map();
    const lines = spec.split("\n");
    for (let i = 0; i < lines.length; i++) {
      const heading = /^### User Story (\d+) - (.+?)(?: \(Priority: P\d+\))?\s*$/.exec(lines[i]);
      if (!heading) continue;
      const scenarios = new Set();
      for (let j = i + 1; j < lines.length && !/^#{1,3} /.test(lines[j]); j++) {
        const item = /^(\d+[a-z]?)\. \*\*Given\*\*/.exec(lines[j]);
        if (item) scenarios.add(item[1]);
      }
      stories.set(heading[1], { title: heading[2], scenarios });
    }
    specs.set(match[1], {
      dir,
      docs,
      stories,
      requirements: new Set([...spec.matchAll(/\*\*((?:FR|SC)-\d+[a-z]?)\*\*/g)].map((m) => m[1])),
      tasks: new Set(
        [...(docs.get("tasks.md") ?? "").matchAll(/^- \[.\] (T\d{3}[a-z]?)\b/gm)].map((m) => m[1]),
      ),
    });
  }
  return specs;
}

/**
 * A spec document's name: `research.md`, `contracts/ui-accessories.md` or
 * just `ui-accessories.md`. It may follow a path into a spec folder, or an ID
 * and a slash (`002 FR-010/research.md §8`), but no other path.
 */
function docPattern(specs) {
  const names = new Set();
  for (const spec of specs.values()) {
    for (const name of spec.docs.keys()) names.add(name.replace(/^(?:contracts|checklists)\//, ""));
  }
  const alternatives = [...names]
    .sort((a, b) => b.length - a.length)
    .map((name) => name.replace(/[.]/g, "\\."));
  const start = String.raw`(?:(?<=specs\/\d{3}-[\w-]+\/(?:contracts\/|checklists\/)?)|(?<![\w.-])(?<![^\d)]\/))`;
  return String.raw`${start}(?:contracts\/|checklists\/)?(?:${alternatives.join("|")})(?![\w-])`;
}

/** The document a cited name stands for in `docs`, if any. */
function findDoc(docs, name) {
  for (const key of [name, `contracts/${name}`, `checklists/${name}`]) {
    if (docs.has(key)) return docs.get(key);
  }
  return undefined;
}

/** Whether `doc` has a heading numbered `section` (`## 5.`, `## §1a`, `### 5.2`). */
function hasSection(doc, section) {
  const number = section.replace(/^§ ?/, "");
  const escaped = number.replace(/[.]/g, "\\.");
  return new RegExp(String.raw`^#+ (?:§ ?)?${escaped}(?:[.:)]| |$)`, "m").test(doc);
}

/**
 * The text as one line, with each line break and the comment markers that
 * start the next line replaced by a space, so a list can wrap, or by nothing
 * after a slash or a word's hyphen, so a path can; and the line each offset
 * is on.
 */
function flatten(text) {
  let flat = "";
  const lineAt = [];
  const lines = text.split("\n");
  lines.forEach((line, index) => {
    let body = line;
    if (index > 0) {
      body = body.replace(/^\s*(?:\/\/[/!]?|\/?\*+(?!\/)|#+|--|<!--|;+)?\s*/, "");
      if (!/(?:\/|[A-Za-z]-)$/.test(flat)) {
        flat += " ";
        lineAt.push(index + 1);
      }
    }
    flat += body;
    for (let i = 0; i < body.length; i++) lineAt.push(index + 1);
  });
  return { flat, lineAt };
}

/** What's wrong with the citations in one file's text, each with its line. */
export function checkText(text, specs) {
  const { flat, lineAt } = flatten(text);
  const atoms = new RegExp(
    String.raw`(?<prose>${PROSE})|(?<id>\b(?:${ID})\b)|(?<doc>${docPattern(specs)})|(?<section>${SECTION})`,
    "g",
  );
  const problems = [];
  let previous = null; // { end, spec, kind, doc }
  for (const match of flat.matchAll(atoms)) {
    const kind = Object.keys(match.groups).find((key) => match.groups[key] !== undefined);
    const token = match[0];
    const start = match.index;
    const end = start + token.length;
    const line = lineAt[Math.min(start, lineAt.length - 1)];
    const before = flat.slice(Math.max(0, start - 200), start);
    if (kind === "prose") {
      const message = /^spec/.test(token)
        ? `write "${token}" as "<spec> spec.md ${token.replace(/^spec(?:'s)? /, "")}"`
        : `write "${token}" as a spec-qualified US citation`;
      problems.push({ line, token, message });
      previous = null;
      continue;
    }
    let spec = null;
    let doc = null;
    const qualifier = /\b(\d{3}) $/.exec(before);
    const inPath = /specs\/(\d{3})-[\w-]+\/(?:contracts\/|checklists\/)?$/.exec(before);
    if (qualifier && specs.has(qualifier[1])) {
      spec = qualifier[1];
    } else if (kind === "doc" && inPath && specs.has(inPath[1])) {
      spec = inPath[1];
    } else if (previous && JOINER.test(flat.slice(previous.end, start))) {
      spec = previous.spec;
      if (kind === "section") doc = previous.doc;
    }
    if (kind === "section" && spec && !doc) spec = null;
    if (!spec && kind === "section" && OTHER_DOCUMENT.test(before)) {
      previous = null;
      continue;
    }
    if (!spec) {
      problems.push({
        line,
        token,
        message:
          kind === "section"
            ? `"${token}" doesn't follow a spec-qualified document`
            : `"${token}" doesn't name its spec`,
      });
      previous = null;
      continue;
    }
    const where = specs.get(spec);
    const missing = (what) =>
      problems.push({ line, token, message: `${spec} ${token}: no ${what} in specs/${where.dir}` });
    if (kind === "doc") {
      doc = token;
      if (findDoc(where.docs, token) === undefined) missing("such document");
    } else if (kind === "section") {
      if (!hasSection(findDoc(where.docs, doc) ?? "", token)) missing(`section ${token} in ${doc}`);
    } else if (/^(?:FR|SC)-/.test(token)) {
      if (!where.requirements.has(token)) missing("such requirement");
    } else if (token.startsWith("US")) {
      const [, story, scenario] = /^US(\d+)(?:-(\w+))?$/.exec(token);
      const found = where.stories.get(story);
      if (!found) missing(`User Story ${story}`);
      else if (scenario && !found.scenarios.has(scenario))
        missing(`scenario ${scenario} in User Story ${story}`);
    } else if (!where.tasks.has(token)) {
      missing("such task");
    }
    previous = { end, spec, kind, doc };
  }
  return problems;
}

/**
 * What's wrong with an E2E spec file's top-level `describe()` titles: each
 * names a story, `"<spec> US<n> - <the spec's heading>"`, optionally followed
 * by `" (was <old file name>)"`, or a regression, `"Issue #<n> - …"`; and at
 * least one names a story.
 */
export function checkE2eDescribes(text, specs) {
  const problems = [];
  let stories = 0;
  const titles = [...text.matchAll(/^describe\(\s*(["'`])(.*?)\1/gm)];
  for (const match of titles) {
    const title = match[2];
    const line = text.slice(0, match.index).split("\n").length;
    if (/^Issue #\d+ - \S/.test(title)) continue;
    const story = /^(\d{3}) US(\d+) - (.+?)(?: \(was [\w-]+\))?$/.exec(title);
    const found = story && specs.get(story[1])?.stories.get(story[2]);
    if (!story) {
      problems.push({
        line,
        token: title,
        message: `describe() title isn't "<spec> US<n> - <heading>" or "Issue #<n> - …"`,
      });
    } else if (!found) {
      problems.push({ line, token: title, message: `${story[1]} has no User Story ${story[2]}` });
    } else if (found.title !== story[3]) {
      problems.push({
        line,
        token: title,
        message: `${story[1]} US${story[2]}'s heading is "${found.title}"`,
      });
    } else {
      stories++;
    }
  }
  if (stories === 0) {
    problems.push({ line: 1, token: "", message: "no describe() names a story" });
  }
  return problems;
}

/** The files checked: tracked text files under `CHECKED_DIRS`. */
export function checkedFiles(root = ROOT) {
  return execFileSync("git", ["-C", root, "ls-files", "-z", "--", ...CHECKED_DIRS], {
    encoding: "utf8",
  })
    .split("\0")
    .filter(
      (file) => file && !EXEMPT.has(file) && !EXEMPT_DIRS.some((dir) => file.startsWith(dir)),
    );
}

/** Every problem in `files` (relative to `root`), as `file:line: message`. */
export function check(files = checkedFiles(), root = ROOT) {
  const specs = loadSpecs(root);
  const report = [];
  for (const file of files) {
    let text;
    try {
      const bytes = fs.readFileSync(path.join(root, file));
      if (bytes.includes(0)) continue;
      text = bytes.toString("utf8");
    } catch {
      continue;
    }
    const problems = checkText(text, specs);
    if (/^e2e\/specs\/[^/]+\.e2e\.ts$/.test(file) && !E2E_HARNESS.has(file)) {
      problems.push(...checkE2eDescribes(text, specs));
    }
    for (const p of problems) report.push(`${file}:${p.line}: ${p.message}`);
  }
  return report;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const named = process.argv.slice(2).map((file) => path.relative(ROOT, path.resolve(file)));
  const report = check(named.length > 0 ? named : undefined);
  for (const line of report) console.log(line);
  console.log(`${report.length} problem${report.length === 1 ? "" : "s"}`);
  process.exitCode = report.length > 0 ? 1 : 0;
}
