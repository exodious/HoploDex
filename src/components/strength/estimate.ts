import { ZxcvbnFactory } from "@zxcvbn-ts/core";
import { adjacencyGraphs } from "@zxcvbn-ts/language-common";
import diceware from "@zxcvbn-ts/language-common/dist/diceware.json.mjs";
import passwords from "@zxcvbn-ts/language-common/dist/passwords.json.mjs";
import { translations } from "@zxcvbn-ts/language-en";
import commonWords from "@zxcvbn-ts/language-en/dist/commonWords.json.mjs";
import firstnames from "@zxcvbn-ts/language-en/dist/firstnames.json.mjs";
import lastnames from "@zxcvbn-ts/language-en/dist/lastnames.json.mjs";
import wordSequences from "@zxcvbn-ts/language-en/dist/wordSequences.json.mjs";

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

/** The word lists the hint looks words up in: the packages' own
 * dictionaries less `wikipedia-en`, whose license we can't meet (010
 * research.md §2, 010 FR-015). The packages' entry modules import every list,
 * so leaving a key out after spreading their `dictionary` would still bundle
 * it; each kept list is imported from its own module instead, and the keys
 * are the ones the entry modules give. */
export function strengthDictionary(): Record<string, string[]> {
  return {
    "commonWords-en": commonWords,
    "firstnames-en": firstnames,
    "lastnames-en": lastnames,
    ...wordSequences,
    "passwords-common": passwords,
    "diceware-common": diceware,
  };
}

/** Builds zxcvbn with its dictionaries and returns the check. Building it
 * turns the dictionaries into ranked lookups, about a second's work, so it
 * runs only in the strength worker (worker.ts), never on the page's main
 * thread (#102). Import this module's types elsewhere with `import type`. */
export function createEstimator(): (value: string) => Estimate {
  const factory = new ZxcvbnFactory({
    translations,
    graphs: adjacencyGraphs,
    dictionary: strengthDictionary(),
  });
  return (value) => {
    const { score, feedback } = factory.check(value);
    return { score, suggestion: feedback.warning || feedback.suggestions[0] || "" };
  };
}
