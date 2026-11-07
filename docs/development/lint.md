# Linting & formatting

_Part of the [development docs](../../DEVELOPMENT.md)._

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml --features test-kdf -- -D warnings
cargo clippy --lib --bins --examples --manifest-path src-tauri/Cargo.toml -- -D warnings   # the build without the test hooks
npm run lint             # eslint, --max-warnings 0
npm run format:check     # prettier
```

Clippy takes `--features test-kdf` because the upgrade's tests import
`db::upgrade::testing`, which exists only with `upgrade-test-steps` (009
research.md §14), and `test-kdf` turns that on; without it `--all-targets`
doesn't compile.

ESLint's `@typescript-eslint/no-deprecated` fails on a use, in the project's
TypeScript (`**/*.{ts,tsx}`), of an API that its type declarations mark
deprecated. Plain-JavaScript scripts (`.mjs`) are not covered. It reads the TypeScript types, so
`npm run lint` takes about 14 s, not about 4 s. A use that has to stay carries
a disable comment with its reason at that line:

```ts
// eslint-disable-next-line @typescript-eslint/no-deprecated -- <reason>
```

Today's three are in `src/components/Combobox.tsx`, `ConfirmDialog.tsx` and
`lock.ts`. Rust needs no extra rule: `deprecated` warnings already fail through
clippy's `-D warnings` (012 research.md §7).
