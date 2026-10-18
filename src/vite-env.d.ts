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

// The program block the build compiles into the interface with Vite `define`
// (010 data-model.md, "Program block"; contracts/notices.md, "The Vite
// plugin").
declare const __HOPLODEX_PROGRAM__: {
  name: string;
  version: string;
  copyrightYears: string;
  copyrightHolder: string;
  sourceUrl: string;
};
