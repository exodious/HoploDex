manifest-valid, derived as 011 data-model.md "The check manifest" allows: the
cargo command's `--profile e2e` and `--features custom-protocol,e2e`, the
binary's path under `target/e2e/`, and no `check-no-webdriver.mjs` step.
`manifest.test.ts` must find no difference from manifest-valid (011 US5-2).
