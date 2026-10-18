// The tiny app (010 tasks.md T015): the main bundle imports two shipped
// packages and starts the module worker. A test aliases each package to its
// folder in ../project/modules, which stands for node_modules.
import { name as mit } from "shipped-mit";
import { name as scoped } from "@scope/shipped-scoped";

console.log(mit, scoped, __HOPLODEX_PROGRAM__);
new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
