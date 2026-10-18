// The dev package reaches the bundle only through the worker
import { name } from "shipped-mit";

console.log(name);
new Worker(new URL("./dev-worker.js", import.meta.url), { type: "module" });
