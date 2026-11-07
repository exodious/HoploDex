# Quickstart: Troubleshooting Log

Run everything in the sandbox tooling (`docs/development/test-isolation.md`), never the app on real data.

1. `cargo nextest run --manifest-path src-tauri/Cargo.toml --features test-kdf -E 'binary(troubleshooting_log_test) | binary(log_contents_test) | binary(log_lines_test)'`.
2. `npm run build`, then the E2E journey in `databases.e2e.ts`.
3. In `npm run session`: clear `HOPLODEX_LOG`, start, read the notice, open About, raise, open a database, read the file in the sandbox's cache folder.
4. Flatpak: `scripts/dev-container.sh --flatpak npm run test:flatpak`; the check reads the file at the Flatpak's cache path and shows it through the portal.
