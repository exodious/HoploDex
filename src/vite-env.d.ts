/// <reference types="vite/client" />

// The zxcvbn-ts packages' per-list modules ship without declarations. Each
// default-exports its list; `wordSequences` is an object of lists (010
// research.md §2).
declare module "@zxcvbn-ts/language-en/dist/wordSequences.json.mjs" {
  const lists: Record<string, string[]>;
  export default lists;
}
declare module "@zxcvbn-ts/language-en/dist/*.json.mjs" {
  const list: string[];
  export default list;
}
declare module "@zxcvbn-ts/language-common/dist/*.json.mjs" {
  const list: string[];
  export default list;
}
