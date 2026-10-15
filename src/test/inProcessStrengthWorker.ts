import { createEstimator } from "../components/strength/estimate";
import type { StrengthAnswer, StrengthRequest } from "../components/strength/estimate";

/** The strength worker for jsdom, which has no workers: answers each request
 * as worker.ts does, but in this thread and a turn of the event loop later,
 * as a worker's answer would arrive. Building zxcvbn waits for the first
 * request, so a test that never types doesn't pay for it. */
export class InProcessStrengthWorker {
  onmessage: ((event: MessageEvent<StrengthAnswer>) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  onmessageerror: (() => void) | null = null;
  private check: ReturnType<typeof createEstimator> | null = null;

  postMessage(request: StrengthRequest) {
    setTimeout(() => {
      this.check ??= createEstimator();
      const answer = { id: request.id, estimate: this.check(request.value) };
      this.onmessage?.({ data: answer } as MessageEvent<StrengthAnswer>);
    });
  }

  terminate() {}
}
