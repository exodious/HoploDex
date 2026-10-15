import { ZxcvbnFactory } from "@zxcvbn-ts/core";
import { adjacencyGraphs, dictionary as commonDictionary } from "@zxcvbn-ts/language-common";
import { dictionary as enDictionary, translations } from "@zxcvbn-ts/language-en";

/** What the strength hint shows for a passphrase. */
export interface Estimate {
  score: number;
  suggestion: string;
}

/** A passphrase sent to the strength worker, and its answer. */
export interface StrengthRequest {
  id: number;
  value: string;
}

export interface StrengthAnswer {
  id: number;
  estimate: Estimate;
}

/** Builds zxcvbn with its dictionaries and returns the check. Building it
 * turns the dictionaries into ranked lookups, about a second's work, so it
 * runs only in the strength worker (worker.ts), never on the page's main
 * thread (#102). Import this module's types elsewhere with `import type`. */
export function createEstimator(): (value: string) => Estimate {
  const factory = new ZxcvbnFactory({
    translations,
    graphs: adjacencyGraphs,
    dictionary: { ...commonDictionary, ...enDictionary },
  });
  return (value) => {
    const { score, feedback } = factory.check(value);
    return { score, suggestion: feedback.warning || feedback.suggestions[0] || "" };
  };
}
