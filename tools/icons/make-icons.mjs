// Generates the program icon's files in src-tauri/icons from tools/icons/AppIcon.tsx
// (issue #22): every PNG size, icon.ico and icon.icns. `npm run icons`.
//
// AppIcon is rendered to SVG through Vite (it is TSX, and shares the chooser
// plate's shield), and each size is rasterized by `tauri icon`'s resvg on its
// own, so the small sizes get their own drawing and thicker lines. The .ico
// and .icns are then packed here from those PNGs.
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";

const root = fileURLToPath(new URL("../..", import.meta.url));
const out = join(root, "src-tauri/icons");
const tauri = join(root, "node_modules/.bin/tauri");

/** Every size any file needs; 32 and below get the small drawing. */
const SIZES = [16, 32, 48, 64, 128, 256, 512, 1024];

/** The PNGs tauri.conf.json lists, by size. icon.png is also the 512. */
const PNGS = {
  "32x32.png": 32,
  "64x64.png": 64,
  "128x128.png": 128,
  "128x128@2x.png": 256,
  "512x512.png": 512,
  "icon.png": 512,
};

/** Windows: the sizes Explorer and the taskbar use. */
const ICO = [16, 32, 48, 64, 256];

/** macOS: each .icns entry's type and size (the @2x types are Retina). */
const ICNS = [
  ["icp4", 16],
  ["icp5", 32],
  ["ic11", 32], // 16@2x
  ["icp6", 64],
  ["ic12", 64], // 32@2x
  ["ic07", 128],
  ["ic08", 256],
  ["ic13", 256], // 128@2x
  ["ic09", 512],
  ["ic14", 512], // 256@2x
  ["ic10", 1024], // 512@2x
];

async function renderSvgs(dir) {
  const server = await createServer({
    root,
    configFile: false,
    logLevel: "error",
    appType: "custom",
    server: { middlewareMode: true, hmr: false },
    optimizeDeps: { noDiscovery: true, include: [] },
  });
  try {
    const { AppIcon } = await server.ssrLoadModule("/tools/icons/AppIcon.tsx");
    for (const size of SIZES) {
      const svg = renderToStaticMarkup(createElement(AppIcon, { size }));
      writeFileSync(
        join(dir, `icon-${size}.svg`),
        `<?xml version="1.0" encoding="UTF-8"?>\n${svg}\n`,
      );
    }
  } finally {
    await server.close();
  }
}

function rasterize(dir) {
  const png = {};
  for (const size of SIZES) {
    const target = join(dir, `png-${size}`);
    execFileSync(
      tauri,
      ["icon", join(dir, `icon-${size}.svg`), "--png", String(size), "-o", target],
      {
        stdio: "ignore",
      },
    );
    const [file] = readdirSync(target).filter((name) => name.endsWith(".png"));
    png[size] = readFileSync(join(target, file));
  }
  return png;
}

/** An .ico of PNG images. */
function ico(png) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2); // an icon
  header.writeUInt16LE(ICO.length, 4);
  const entries = [];
  let offset = 6 + 16 * ICO.length;
  for (const size of ICO) {
    const entry = Buffer.alloc(16);
    entry.writeUInt8(size >= 256 ? 0 : size, 0); // 0 means 256
    entry.writeUInt8(size >= 256 ? 0 : size, 1);
    entry.writeUInt16LE(1, 4); // colour planes
    entry.writeUInt16LE(32, 6); // bits per pixel
    entry.writeUInt32LE(png[size].length, 8);
    entry.writeUInt32LE(offset, 12);
    offset += png[size].length;
    entries.push(entry);
  }
  return Buffer.concat([header, ...entries, ...ICO.map((size) => png[size])]);
}

/** An .icns of PNG images. */
function icns(png) {
  const chunks = ICNS.map(([type, size]) => {
    const head = Buffer.alloc(8);
    head.write(type, 0, "ascii");
    head.writeUInt32BE(8 + png[size].length, 4);
    return Buffer.concat([head, png[size]]);
  });
  const body = Buffer.concat(chunks);
  const head = Buffer.alloc(8);
  head.write("icns", 0, "ascii");
  head.writeUInt32BE(8 + body.length, 4);
  return Buffer.concat([head, body]);
}

const dir = mkdtempSync(join(tmpdir(), "hoplodex-icons-"));
try {
  await renderSvgs(dir);
  const png = rasterize(dir);
  for (const [name, size] of Object.entries(PNGS)) writeFileSync(join(out, name), png[size]);
  writeFileSync(join(out, "icon.ico"), ico(png));
  writeFileSync(join(out, "icon.icns"), icns(png));
  // the artwork itself, for reference: the large drawing and the small one
  copyFileSync(join(dir, "icon-512.svg"), join(out, "source/icon.svg"));
  copyFileSync(join(dir, "icon-32.svg"), join(out, "source/icon-small.svg"));
  console.log(`Wrote ${Object.keys(PNGS).join(", ")}, icon.ico and icon.icns to src-tauri/icons.`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
