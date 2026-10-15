import type { Estimate, StrengthAnswer, StrengthRequest } from "./estimate";
import { spawnStrengthWorker } from "./spawn";

/** The page's side of the strength worker (worker.ts, #102). One worker
 * serves the app for as long as it runs, as the built zxcvbn used to be kept.
 *
 * An estimate isn't counted in `src/lib/busy.ts`: the hint only advises, and
 * the end-to-end tests that look at it wait for it themselves, so the others
 * needn't wait for zxcvbn's dictionaries. */

let worker: Worker | null = null;
let failed = false;
let nextId = 0;
const waiting = new Map<number, (estimate: Estimate | null) => void>();

function fail(error: unknown) {
  if (failed) return;
  failed = true;
  console.error("The passphrase strength hint is unavailable:", error);
  worker?.terminate();
  worker = null;
  for (const answer of waiting.values()) answer(null);
  waiting.clear();
}

/** Starts the worker if it isn't running, so zxcvbn is built while the user
 * fills in the fields above the passphrase. */
export function startStrengthWorker() {
  if (worker || failed) return;
  try {
    worker = spawnStrengthWorker();
  } catch (error) {
    fail(error);
    return;
  }
  worker.onmessage = ({ data }: MessageEvent<StrengthAnswer>) => {
    waiting.get(data.id)?.(data.estimate);
    waiting.delete(data.id);
  };
  worker.onerror = (event) => {
    event.preventDefault();
    fail(event.message || "the strength worker failed");
  };
  worker.onmessageerror = () => fail("the strength worker's answer could not be read");
}

/** zxcvbn's estimate for `value`, or `null` when the worker is unavailable
 * (the hint then shows only its advice: it never blocks, FR-003). The value
 * is posted to the worker and kept nowhere here. */
export function estimateStrength(value: string): Promise<Estimate | null> {
  startStrengthWorker();
  const running = worker;
  if (!running) return Promise.resolve(null);
  const id = ++nextId;
  const request: StrengthRequest = { id, value };
  return new Promise((resolve) => {
    waiting.set(id, resolve);
    running.postMessage(request);
  });
}
