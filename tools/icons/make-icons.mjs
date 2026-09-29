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
import { inflateSync } from "node:zlib";
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

/**
 * macOS: each .icns entry's type and size. The @2x types are Retina. The 16 and
 * 32 px slots of the 1x set (ic04, ic05) hold ARGB runs, not PNG; the Windows
 * style icp4 to icp6 types hold PNG, which macOS reads as raw pixels, so Finder's
 * small views showed noise. Larger types and the @2x ones are PNG.
 */
const ICNS = [
  ["ic04", 16, "argb"],
  ["ic05", 32, "argb"],
  ["ic11", 32], // 16@2x
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

/** The RGBA pixels of an 8-bit, non-interlaced RGBA PNG (what resvg writes). */
function decodePng(buf) {
  const width = buf.readUInt32BE(16);
  const height = buf.readUInt32BE(20);
  if (buf[24] !== 8 || buf[25] !== 6 || buf[28] !== 0) {
    throw new Error("expected an 8-bit RGBA PNG without interlacing");
  }
  const idat = [];
  for (let at = 8; at < buf.length;) {
    const length = buf.readUInt32BE(at);
    if (buf.toString("ascii", at + 4, at + 8) === "IDAT") {
      idat.push(buf.subarray(at + 8, at + 8 + length));
    }
    at += 12 + length;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * 4;
  const pixels = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const left = x >= 4 ? pixels[y * stride + x - 4] : 0;
      const up = y > 0 ? pixels[(y - 1) * stride + x] : 0;
      const upLeft = x >= 4 && y > 0 ? pixels[(y - 1) * stride + x - 4] : 0;
      let predictor = 0;
      if (filter === 1) predictor = left;
      else if (filter === 2) predictor = up;
      else if (filter === 3) predictor = (left + up) >> 1;
      else if (filter === 4) {
        const p = left + up - upLeft;
        const [a, b, c] = [Math.abs(p - left), Math.abs(p - up), Math.abs(p - upLeft)];
        predictor = a <= b && a <= c ? left : b <= c ? up : upLeft;
      }
      pixels[y * stride + x] = (line[x] + predictor) & 255;
    }
  }
  return pixels;
}

/** One channel, run-length coded as icns does: a byte n < 128 is followed by
 * n + 1 literal bytes; a byte 128 + n by one byte repeated n + 3 times. */
function rle(channel) {
  const out = [];
  let literal = [];
  const flush = () => {
    for (let at = 0; at < literal.length; at += 128) {
      const part = literal.slice(at, at + 128);
      out.push(part.length - 1, ...part);
    }
    literal = [];
  };
  for (let i = 0; i < channel.length;) {
    let run = 1;
    while (i + run < channel.length && channel[i + run] === channel[i] && run < 130) run++;
    if (run >= 3) {
      flush();
      out.push(125 + run, channel[i]);
      i += run;
    } else {
      literal.push(channel[i++]);
    }
  }
  flush();
  return Buffer.from(out);
}

/** An icns "ARGB" entry: the magic, then the alpha, red, green and blue channels. */
function argb(png) {
  const pixels = decodePng(png);
  const channels = [3, 0, 1, 2].map((offset) =>
    Uint8Array.from({ length: pixels.length / 4 }, (_, i) => pixels[i * 4 + offset]),
  );
  return Buffer.concat([Buffer.from("ARGB", "ascii"), ...channels.map(rle)]);
}

/** An .icns of PNG images (and ARGB runs for the 1x small sizes). */
function icns(png) {
  const chunks = ICNS.map(([type, size, format]) => {
    const data = format === "argb" ? argb(png[size]) : png[size];
    const head = Buffer.alloc(8);
    head.write(type, 0, "ascii");
    head.writeUInt32BE(8 + data.length, 4);
    return Buffer.concat([head, data]);
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
