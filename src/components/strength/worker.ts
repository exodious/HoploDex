import { createEstimator } from "./estimate";
import type { StrengthAnswer, StrengthRequest } from "./estimate";

/** The strength worker (#102): zxcvbn is built and run here, off the page's
 * main thread, so the Create database dialog never freezes while its
 * dictionaries load. The worker is part of the page, so the passphrase still
 * never crosses IPC (003 research.md §18). It answers each request and keeps
 * nothing of it. */

const scope = self as unknown as {
  onmessage: ((event: MessageEvent<StrengthRequest>) => void) | null;
  postMessage(answer: StrengthAnswer): void;
};

// Built as soon as the worker starts; requests sent meanwhile wait in its
// queue.
const check = createEstimator();

scope.onmessage = ({ data }) => scope.postMessage({ id: data.id, estimate: check(data.value) });
