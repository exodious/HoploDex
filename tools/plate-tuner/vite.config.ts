import { randomBytes } from "node:crypto";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import type { Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { saveHandler } from "./saveRequest";
import { TOKEN_META } from "./token";

/*
 * The chooser plate's tuner: `npm run tuner` serves it, with a Save button
 * that writes your changes into timing.ts; `npm run tuner:build` builds it
 * as one self-contained page, dist-tuner/plate-tuner.html, to open anywhere
 * (without Save: copy the changes out instead).
 */

const PORT = 1430;
const here = fileURLToPath(new URL(".", import.meta.url));
const TIMING = fileURLToPath(
  new URL("../../src/features/databases/plate/timing.ts", import.meta.url),
);

/**
 * POST /__plate-timing with the changed values writes them into timing.ts,
 * for the tuner page this server serves and nothing else: see
 * saveRequest.ts. The page gets this run's token from a <meta> in its HTML.
 */
function saveTiming(): Plugin {
  const token = randomBytes(32).toString("hex");
  return {
    name: "plate-tuner-save",
    apply: "serve",
    transformIndexHtml: () => [
      { tag: "meta", attrs: { name: TOKEN_META, content: token }, injectTo: "head" },
    ],
    configureServer(server) {
      server.middlewares.use(
        "/__plate-timing",
        saveHandler({
          token,
          timingPath: TIMING,
          port: () => {
            const address = server.httpServer?.address();
            return typeof address === "object" && address ? address.port : PORT;
          },
        }),
      );
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
  server: {
    port: PORT,
    // loopback only, whatever the environment says
    host: "localhost",
    // Vite lets other localhost origins read what it serves; the page's
    // Save token must not be readable by them
    cors: false,
    fs: { allow: [fileURLToPath(new URL("../..", import.meta.url))] },
  },
  build: {
    outDir: fileURLToPath(new URL("../../dist-tuner", import.meta.url)),
    emptyOutDir: true,
    assetsInlineLimit: 100_000_000,
    cssCodeSplit: false,
  },
});
