/** Starts the strength worker. On its own so the unit tests, which run in
 * jsdom without workers, can answer in-process instead (src/test/setup.ts). */
export function spawnStrengthWorker(): Worker {
  return new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
}
