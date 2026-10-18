// A withdrawn data file in the main bundle
import data from "./withdrawn.json";

console.log(data);
new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
