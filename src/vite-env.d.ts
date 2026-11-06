/// <reference types="vite/client" />

// The program block the build compiles into the interface with Vite `define`
// (010 data-model.md, "Program block"; 010 contracts/notices.md, "The Vite
// plugin").
declare const __HOPLODEX_PROGRAM__: {
  name: string;
  version: string;
  copyrightYears: string;
  copyrightHolder: string;
  sourceUrl: string;
};
