import { timingSafeEqual } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";
import { writeTiming } from "./saveTiming";
import { TOKEN_HEADER } from "./token";

/*
 * The tuner's `POST /__plate-timing`, which rewrites a source file, so it
 * answers only the tuner's own page (issue #70):
 *
 *  - the page carries a token the server made at start-up (injected into
 *    the HTML it serves, which no other site can read) and sends it in a
 *    custom header, which also makes a browser preflight a cross-site
 *    request, a preflight this server never grants;
 *  - `Host` and, when sent, `Origin` have to be the tuner's own loopback
 *    address (the first stops DNS rebinding, the second a foreign page);
 *  - the body is JSON, at most `MAX_BODY` bytes, and passes the schema
 *    (timingSchema.ts) before anything is written.
 *
 * Every refusal comes before the file is read or written.
 */

export const MAX_BODY = 64 * 1024;

export interface SaveOptions {
  /** The per-run secret the tuner page must send. */
  token: string;
  /** The file to rewrite. */
  timingPath: string;
  /** The port the server is listening on, once it is. */
  port: () => number;
}

function refuse(res: ServerResponse, status: number, message: string) {
  res.statusCode = status;
  res.setHeader("Content-Type", "text/plain; charset=utf-8");
  res.end(message);
}

/** The loopback names the tuner is reached by. */
function ownOrigins(port: number): string[] {
  return ["localhost", "127.0.0.1", "[::1]"].map((host) => `http://${host}:${port}`);
}

function tokenMatches(sent: string | string[] | undefined, token: string): boolean {
  if (typeof sent !== "string") return false;
  const a = Buffer.from(sent);
  const b = Buffer.from(token);
  return a.length === b.length && timingSafeEqual(a, b);
}

/** The request's refusal, as [status, message], or null if it may go on to
 * its body. */
export function checkSaveRequest(
  req: Pick<IncomingMessage, "method" | "headers">,
  options: Pick<SaveOptions, "token" | "port">,
): [number, string] | null {
  const { headers } = req;
  if (req.method !== "POST") return [405, "POST only"];
  const origins = ownOrigins(options.port());
  if (typeof headers.host !== "string" || !origins.includes(`http://${headers.host}`))
    return [403, "not the tuner's own address"];
  if (headers.origin !== undefined && !origins.includes(headers.origin))
    return [403, "not the tuner's own origin"];
  if (!tokenMatches(headers[TOKEN_HEADER], options.token))
    return [403, "missing or wrong tuner token: reload the tuner page"];
  const type = (headers["content-type"] ?? "").split(";")[0].trim().toLowerCase();
  if (type !== "application/json") return [415, "send application/json"];
  if (Number(headers["content-length"] ?? 0) > MAX_BODY) return [413, "body too large"];
  return null;
}

/** Reads the body, giving up past `MAX_BODY` bytes (the connection is cut). */
function readBody(req: IncomingMessage): Promise<string | null> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let size = 0;
    req.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > MAX_BODY) {
        chunks.length = 0;
        req.removeAllListeners("data");
        req.resume();
        resolve(null);
        return;
      }
      chunks.push(chunk);
    });
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}

/** The connect middleware for `/__plate-timing`. */
export function saveHandler(options: SaveOptions) {
  return (req: IncomingMessage, res: ServerResponse) => {
    const refusal = checkSaveRequest(req, options);
    if (refusal) {
      // a refused request's body is never read: stop the sender sending it
      res.setHeader("Connection", "close");
      refuse(res, refusal[0], refusal[1]);
      return;
    }
    readBody(req)
      .then((body) => {
        if (body === null) {
          res.setHeader("Connection", "close");
          return refuse(res, 413, "body too large");
        }
        try {
          const source = readFileSync(options.timingPath, "utf8");
          writeFileSync(options.timingPath, writeTiming(source, JSON.parse(body)));
          res.statusCode = 204;
          res.end();
        } catch (error) {
          refuse(res, 400, String(error instanceof Error ? error.message : error));
        }
      })
      .catch(() => refuse(res, 400, "the request was cut off"));
  };
}
