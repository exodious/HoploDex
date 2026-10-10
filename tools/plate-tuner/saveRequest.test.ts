import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer, request } from "node:http";
import type { Server } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import source from "../../src/features/databases/plate/timing.ts?raw";
import { MAX_BODY, saveHandler } from "./saveRequest";
import { TOKEN_HEADER } from "./token";

/*
 * The tuner's Save endpoint (issue #70), run on a real HTTP server against a
 * scratch copy of timing.ts: every refusal has to leave the file as it was.
 */

const TOKEN = "0123456789abcdef".repeat(4);

let dir: string;
let file: string;
let server: Server;
let port: number;

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), "plate-tuner-"));
  file = join(dir, "timing.ts");
  server = createServer(saveHandler({ token: TOKEN, timingPath: file, port: () => port }));
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  port = (server.address() as { port: number }).port;
});
afterAll(async () => {
  await new Promise((done) => server.close(done));
  rmSync(dir, { recursive: true, force: true });
});

interface Sent {
  method?: string;
  headers?: Record<string, string | undefined>;
  body?: string | Buffer;
}

/** Sends a request that would be accepted, with `sent` changed. Resolves with the status. */
function send(sent: Sent = {}): Promise<{ status: number; text: string }> {
  const headers: Record<string, string> = {
    "content-type": "application/json",
    [TOKEN_HEADER]: TOKEN,
    origin: `http://localhost:${port}`,
    host: `localhost:${port}`,
  };
  for (const [name, value] of Object.entries(sent.headers ?? {}))
    if (value === undefined) delete headers[name];
    else headers[name] = value;
  const body = sent.body ?? JSON.stringify({ SPEED: 0.8 });
  return new Promise((resolve, reject) => {
    const req = request({
      port,
      host: "127.0.0.1",
      method: sent.method ?? "POST",
      path: "/",
      headers,
    });
    req.on("response", (res) => {
      let text = "";
      res.on("data", (chunk) => (text += chunk));
      res.on("end", () => resolve({ status: res.statusCode ?? 0, text }));
    });
    // an oversize upload may be cut off by the server's answer
    req.on("error", reject);
    req.end(body);
  });
}

function fresh() {
  writeFileSync(file, source);
}
const unchanged = () => expect(readFileSync(file, "utf8")).toBe(source);

describe("the tuner's Save endpoint", () => {
  it("saves a valid request from the tuner's own page", async () => {
    fresh();
    const { status } = await send({
      body: JSON.stringify({ SPEED: 0.8, RIM_EASE: [0.45, 0, 0.55, 1] }),
    });
    expect(status).toBe(204);
    const out = readFileSync(file, "utf8");
    expect(out).toMatch(/^ {2}SPEED: 0\.8,/m);
    expect(out).toMatch(/^ {2}RIM_EASE: \[0\.45, 0, 0\.55, 1\],/m);
  });

  it("also answers the same page reached as 127.0.0.1", async () => {
    fresh();
    const { status } = await send({
      headers: { host: `127.0.0.1:${port}`, origin: `http://127.0.0.1:${port}` },
    });
    expect(status).toBe(204);
  });

  it("accepts a request with no Origin (the token still guards it)", async () => {
    fresh();
    expect((await send({ headers: { origin: undefined } })).status).toBe(204);
  });

  it.each([
    ["no token", { [TOKEN_HEADER]: undefined }],
    ["a wrong token", { [TOKEN_HEADER]: "x".repeat(TOKEN.length) }],
    ["a token of another length", { [TOKEN_HEADER]: "short" }],
    ["another site's Origin", { origin: "https://evil.example" }],
    ["another port's Origin", { origin: "http://localhost:1" }],
    ["the null Origin", { origin: "null" }],
    ["a Host that isn't loopback (DNS rebinding)", { host: `evil.example:${port}` }],
  ])("refuses %s with 403, leaving the file alone", async (_name, headers) => {
    fresh();
    expect((await send({ headers })).status).toBe(403);
    unchanged();
  });

  it.each([
    ["no content type", undefined],
    ["a form content type (a simple cross-site request)", "application/x-www-form-urlencoded"],
    ["text/plain", "text/plain"],
  ])("refuses %s with 415", async (_name, type) => {
    fresh();
    expect((await send({ headers: { "content-type": type } })).status).toBe(415);
    unchanged();
  });

  it("accepts application/json with a charset", async () => {
    fresh();
    expect(
      (await send({ headers: { "content-type": "application/json; charset=utf-8" } })).status,
    ).toBe(204);
  });

  it("refuses anything but POST", async () => {
    fresh();
    for (const method of ["GET", "PUT", "OPTIONS"])
      expect((await send({ method, body: "" })).status).toBe(405);
    unchanged();
  });

  it("refuses a body over the cap with 413, leaving the file alone", async () => {
    fresh();
    const body = JSON.stringify({ SPEED: 0.8, pad: "x".repeat(MAX_BODY) });
    expect((await send({ body })).status).toBe(413);
    unchanged();
  });

  it("stops reading a body that grows past the cap without announcing its length", async () => {
    fresh();
    const status = await new Promise<number>((resolve, reject) => {
      const req = request({
        port,
        host: "127.0.0.1",
        method: "POST",
        path: "/",
        headers: {
          "content-type": "application/json",
          [TOKEN_HEADER]: TOKEN,
          host: `localhost:${port}`,
          "transfer-encoding": "chunked",
        },
      });
      req.on("response", (res) => {
        res.resume();
        resolve(res.statusCode ?? 0);
      });
      req.on("error", reject);
      req.write('{"SPEED":0.8,"pad":"');
      req.write("x".repeat(MAX_BODY));
      req.end('"}');
    });
    expect(status).toBe(413);
    unchanged();
  });

  it.each([
    ["an array member that is code", { RIM_EASE: [0, 0, "1); alert(1); (", 1] }],
    ["a key outside the schema", { NOT_A_KNOB: 1 }],
    ["a key that isn't a name at all", { "SPEED: 1, //": 1 }],
    ["a string for a number", { SPEED: "1" }],
    ["an out-of-range number", { SPEED: 1e9 }],
    ["an array of the wrong length", { RIM_EASE: [0, 0, 1] }],
    ["an unknown style", { CYCLE_STYLE: 'erase"; alert(1); "' }],
    ["a list instead of an object", [1, 2]],
    ["not JSON at all", "not json"],
  ])("refuses %s with 400, leaving the file alone", async (_name, payload) => {
    fresh();
    const body = typeof payload === "string" ? payload : JSON.stringify(payload);
    expect((await send({ body })).status).toBe(400);
    unchanged();
  });

  it("writes nothing when only one of several values is bad", async () => {
    fresh();
    const body = JSON.stringify({ SPEED: 0.8, KEY_TIME: "2" });
    expect((await send({ body })).status).toBe(400);
    unchanged();
  });
});
