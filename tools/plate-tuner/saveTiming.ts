import type { PlateTiming } from "../../src/features/databases/plate/timing";

/** A value as timing.ts writes it. */
export function literal(value: PlateTiming[keyof PlateTiming]): string {
  if (Array.isArray(value)) return `[${value.join(", ")}]`;
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "boolean") return String(value);
  return String(Number(value.toFixed(3)));
}

/**
 * Writes `values` into timing.ts's source, keeping everything else in it:
 * each `NAME: value,` line has its value replaced and its comment kept. A
 * name the file doesn't have, or has more than once, is an error, so the
 * tuner never saves half its changes.
 */
export function writeTiming(source: string, values: Partial<PlateTiming>): string {
  let out = source;
  for (const [name, value] of Object.entries(values)) {
    if (!/^[A-Z][A-Z0-9_]*$/.test(name)) throw new Error(`${name} isn't a timing name`);
    const line = new RegExp(`^([ \\t]*${name}: )(.+?)(,[ \\t]*(?://.*)?)$`, "m");
    const matches = out.match(new RegExp(line.source, "gm")) ?? [];
    if (matches.length !== 1)
      throw new Error(`timing.ts has ${matches.length} lines for ${name}, not one`);
    out = out.replace(line, (_all, head: string, _old: string, tail: string) => {
      return `${head}${literal(value as PlateTiming[keyof PlateTiming])}${tail}`;
    });
  }
  return out;
}
