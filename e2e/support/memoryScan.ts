import fs from "node:fs";

/**
 * Counts copies of a passphrase in a process's memory (Linux only), for the
 * check that one typed in the native dialog never reaches the web view
 * (amended 2026-11-04 to 003 FR-007). The wdio worker starts the app, so it is
 * an ancestor of the app and of WebKit's processes, which Yama's
 * `ptrace_scope` 1 requires for reading `/proc/<pid>/mem`.
 */

export const HAS_MEMORY_SCAN = process.platform === "linux";

/** The processes started by `pid`, and by them, in turn. */
export function descendants(pid: number): number[] {
  const parents = new Map<number, number[]>();
  for (const entry of fs.readdirSync("/proc")) {
    if (!/^\d+$/.test(entry)) continue;
    let stat: string;
    try {
      stat = fs.readFileSync(`/proc/${entry}/stat`, "utf-8");
    } catch {
      continue;
    }
    // The command name is in parentheses and may hold spaces.
    const parent = Number(stat.slice(stat.lastIndexOf(")") + 2).split(" ")[1]);
    parents.set(parent, [...(parents.get(parent) ?? []), Number(entry)]);
  }
  const found: number[] = [];
  const queue = [...(parents.get(pid) ?? [])];
  while (queue.length > 0) {
    const next = queue.shift()!;
    found.push(next);
    queue.push(...(parents.get(next) ?? []));
  }
  return found;
}

/** The process's name, as `/proc/<pid>/comm` gives it. */
export function commandOf(pid: number): string {
  try {
    return fs.readFileSync(`/proc/${pid}/comm`, "utf-8").trim();
  } catch {
    return "";
  }
}

/** WebKit's web processes among the app's: they render the page. */
export function webProcesses(appPid: number): number[] {
  return descendants(appPid).filter((pid) => commandOf(pid).startsWith("WebKitWebProces"));
}

/** How many times `needle` is in the writable anonymous memory of `pid` (its
 * heaps, where a program's strings are), in UTF-8 and in UTF-16LE. Mapped
 * files and devices are left out, which keeps a scan to seconds; regions that
 * can't be read are skipped. */
export function copiesIn(pid: number, needle: string): number {
  const patterns = [Buffer.from(needle, "utf-8"), Buffer.from(needle, "utf16le")];
  const maps = fs.readFileSync(`/proc/${pid}/maps`, "utf-8").split("\n");
  const mem = fs.openSync(`/proc/${pid}/mem`, "r");
  let found = 0;
  const CHUNK = 1 << 20;
  const overlap = Math.max(...patterns.map((p) => p.length)) - 1;
  try {
    for (const line of maps) {
      const [range, perms, , , , ...rest] = line.split(/\s+/);
      const name = rest.join(" ");
      const anonymous = name === "" || name === "[heap]" || name.startsWith("[anon");
      if (!range || !perms?.startsWith("rw") || !anonymous) continue;
      const [start, end] = range.split("-").map((hex) => parseInt(hex, 16));
      for (let at = start; at < end; at += CHUNK) {
        const length = Math.min(CHUNK + overlap, end - at);
        const buffer = Buffer.alloc(length);
        let read: number;
        try {
          read = fs.readSync(mem, buffer, 0, length, at);
        } catch {
          continue;
        }
        const view = buffer.subarray(0, read);
        for (const pattern of patterns) {
          // A copy across the chunk's end is counted in the next chunk only.
          for (
            let i = view.indexOf(pattern);
            i !== -1 && i < CHUNK;
            i = view.indexOf(pattern, i + 1)
          ) {
            found += 1;
          }
        }
      }
    }
  } finally {
    fs.closeSync(mem);
  }
  return found;
}
