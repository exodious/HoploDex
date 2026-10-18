// A withdrawn data file only in the worker's bundle
import { name } from "shipped-mit";

console.log(name);
new Worker(new URL("./withdrawn-worker.js", import.meta.url), { type: "module" });
