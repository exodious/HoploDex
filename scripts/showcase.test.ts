// @vitest-environment node
// The showcase (docs/development/screenshots.md, "The showcase"): the GitHub
// Pages site in site/ and the README's opening, and the shots in
// docs/showcase/ that `npm run screenshots:showcase` takes of the seed's
// showcase collection.
import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const ROOT = path.resolve(import.meta.dirname, "..");
const read = (file: string) => fs.readFileSync(path.join(ROOT, file), "utf8");

/** The images a page names under `prefix`, as `<img src>` or `<source srcset>`. */
function imagesNamed(text: string, prefix: string): string[] {
  return [...text.matchAll(/(?:src|srcset)="([^"]+)"/g)]
    .map(([, url]) => url)
    .filter((url) => url.startsWith(prefix))
    .map((url) => url.slice(prefix.length));
}

describe("the showcase", () => {
  it("says in the site's footer that its photos are public domain and CC0", () => {
    expect(read("site/index.html")).toContain("Photos: public domain and CC0");
  });

  it("names exactly the shots docs/showcase/ holds, each light and dark", () => {
    const named = new Set([
      ...imagesNamed(read("site/index.html"), "showcase/"),
      ...imagesNamed(read("README.md"), "docs/showcase/"),
    ]);
    const held = fs.readdirSync(path.join(ROOT, "docs/showcase")).filter((f) => f.endsWith(".png"));
    expect([...named].sort()).toEqual(held.sort());
    for (const file of named) {
      const pair = file.endsWith("-light.png")
        ? file.replace(/-light\.png$/, "-dark.png")
        : file.replace(/-dark\.png$/, "-light.png");
      expect(named.has(pair), `${file} has no ${pair}`).toBe(true);
    }
  });
});
