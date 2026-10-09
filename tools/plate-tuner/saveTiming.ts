import type { PlateTiming } from "../../src/features/databases/plate/timing";
import { validateTiming } from "./timingSchema";

function number(value: number): string {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new Error("not a finite number");
  return String(Number(value.toFixed(3)));
}

/**
 * A value as timing.ts writes it. Only numbers, booleans and strings
 * (written as JSON strings) are ever written, whatever the caller passes.
 */
export function literal(value: PlateTiming[keyof PlateTiming]): string {
  if (Array.isArray(value)) return `[${value.map(number).join(", ")}]`;
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "boolean") return String(value);
  return number(value);
}

/**
 * Writes `values` into timing.ts's source, keeping everything else in it:
 * each `NAME: value,` line has its value replaced and its comment kept. A
 * name the file doesn't have, or has more than once, is an error, so the
 * tuner never saves half its changes. `input` is checked first
 * (`validateTiming`), and only what passes is written.
 */
export function writeTiming(source: string, input: Partial<PlateTiming>): string {
  const values = validateTiming(input);
  let out = source;
  for (const [name, value] of Object.entries(values)) {
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
