import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import type { Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { writeTiming } from "./saveTiming";

/*
 * The chooser plate's tuner: `npm run tuner` serves it, with a Save button
 * that writes your changes into timing.ts; `npm run tuner:build` builds it
 * as one self-contained page, dist-tuner/plate-tuner.html, to open anywhere
 * (without Save: copy the changes out instead).
 */

const here = fileURLToPath(new URL(".", import.meta.url));
const TIMING = fileURLToPath(
  new URL("../../src/features/databases/plate/timing.ts", import.meta.url),
);

/** POST /__plate-timing with the changed values writes them into timing.ts. */
function saveTiming(): Plugin {
  return {
    name: "plate-tuner-save",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__plate-timing", (req, res) => {
        if (req.method !== "POST") {
          res.statusCode = 405;
          res.end();
          return;
        }
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", () => {
          try {
            writeFileSync(TIMING, writeTiming(readFileSync(TIMING, "utf8"), JSON.parse(body)));
            res.statusCode = 204;
            res.end();
          } catch (error) {
            res.statusCode = 400;
            res.end(String(error instanceof Error ? error.message : error));
          }
        });
      });
    },
  };
}

/** Puts the built script and stylesheet (fonts included) into the page, so
 * it opens from a file or as an artifact. */
function oneFile(): Plugin {
  return {
    name: "plate-tuner-one-file",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      const page = Object.values(bundle).find((file) => file.fileName.endsWith(".html"));
      if (!page || page.type !== "asset") return;
      let html = String(page.source);
      for (const [name, file] of Object.entries(bundle)) {
        if (file.type === "chunk" && html.includes(name)) {
          const code = file.code.replace(/<\/script/gi, "<\\/script");
          html = html.replace(
            new RegExp(`<script type="module" crossorigin src="[^"]*${name}"></script>`),
            () => `<script type="module">${code}</script>`,
          );
          delete bundle[name];
        } else if (file.type === "asset" && name.endsWith(".css") && html.includes(name)) {
          html = html.replace(
            new RegExp(`<link rel="stylesheet" crossorigin href="[^"]*${name}">`),
            () => `<style>${String(file.source)}</style>`,
          );
          delete bundle[name];
        }
      }
      // named for what it is, once it leaves the tuner's folder
      delete bundle[page.fileName];
      this.emitFile({ type: "asset", fileName: "plate-tuner.html", source: html });
    },
  };
}

export default defineConfig({
  root: here,
  base: "./",
  plugins: [react(), saveTiming(), oneFile()],
  server: { port: 1430, fs: { allow: [fileURLToPath(new URL("../..", import.meta.url))] } },
  build: {
    outDir: fileURLToPath(new URL("../../dist-tuner", import.meta.url)),
    emptyOutDir: true,
    assetsInlineLimit: 100_000_000,
    cssCodeSplit: false,
  },
});
