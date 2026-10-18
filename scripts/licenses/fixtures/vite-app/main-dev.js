// A package the fixture lockfile marks dev, bundled into the main bundle
import { name as tool } from "dev-tool";

console.log(tool);
new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
