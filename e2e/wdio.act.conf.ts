import { config as base } from "./wdio.conf";

/**
 * #89 trial only (a throwaway branch): wdio.conf.ts plus WebdriverIO 10's
 * `@wdio/ai-service`, for the specs in act-specs/, which use `browser.act()`.
 *
 *   HOPLODEX_ACT_MODEL   `llama-cpp:<name>` (with HOPLODEX_ACT_BASE_URL, the
 *                        server's OpenAI-compatible `/v1` URL) or
 *                        `anthropic:<model>` (with ANTHROPIC_API_KEY).
 *                        Unset: replay only, the model is never called.
 *   HOPLODEX_ACT_CACHE   the service's `cache` mode; default `locked` without
 *                        a model and `write` with one.
 */
const model = process.env.HOPLODEX_ACT_MODEL;
const baseURL = process.env.HOPLODEX_ACT_BASE_URL;
const cache = (process.env.HOPLODEX_ACT_CACHE ?? (model ? "write" : "locked")) as
  "write" | "heal" | "locked" | "off";

function modelOption() {
  if (!model) return undefined;
  const [provider, ...rest] = model.split(":");
  const name = rest.join(":");
  if (provider === "llama-cpp") {
    return { provider, model: name, baseURL, temperature: 0, maxTokens: 4096 };
  }
  return model;
}

export const config: WebdriverIO.Config = {
  ...base,
  specs: ["./act-specs/**/*.e2e.ts"],
  services: [
    [
      "ai",
      {
        model: modelOption(),
        cache,
        maxSteps: 25,
        workspace: { keep: "always" },
      },
    ],
  ],
};
