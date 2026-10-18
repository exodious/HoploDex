# Contract: Backend Interface Changes

**Feature**: [spec.md](../spec.md) | **Research**: [research.md](../research.md) §10, §11

## Commands

None added, changed or removed. About HoploDex calls no command. The notices
are a static file of the page (`notices.json`), fetched from the app's own
origin, and the program block is compiled into the interface. So
`commands_list.rs`, `capabilities/default.json` and `generate_handler!` are
unchanged, and `acl_manifest_test.rs` has nothing new to check.

## Events

| Event | Direction | Payload | When |
|---|---|---|---|
| `app:about-requested` | backend → main web view | none (`()`) | macOS only: the application menu's "About HoploDex" item (id `about`) is chosen |

- **Emitted by** `main.rs`'s `on_menu_event`, which asks `app_menu::requested_event`
  for the event of the chosen item's id (`about` gives `app:about-requested`,
  any other id none) and emits it with `app.emit(name, ())`, beside
  `request_quit`'s `app:quit-requested`.
- **The menu**: on macOS, `setup()` sets the menu `app_menu::build` makes
  (`src-tauri/src/app_menu.rs`): `Menu::default(app)` with the application
  submenu's predefined About item replaced, at the same position, by
  `MenuItem::with_id(app, "about", "About HoploDex", true, None)`. If the
  default menu isn't shaped as expected (the first item isn't the predefined
  About), it is left as Tauri builds it and a warning is logged. On Linux and
  Windows the app sets no menu, as today. The item id and event names, and the
  mapping between them, compile on every OS and are unit-tested there.
- **Permissions**: listening needs `core:event:allow-listen`, which
  `core:default` already grants. No capability changes.
- **Frontend**: `sessionService.onAboutRequested(handler)` wraps `listen`, and
  `src/test/setup.ts`'s `listen` mock covers it in Vitest.

## The `hdpreview` protocol, the CSP and plugins

Unchanged. `notices.json` and `NOTICES.txt` are served by Tauri's own asset
protocol like every other file in `dist/`. `tests/csp_test.rs`'s rules (no
network plugin, no `opener:allow-open-url`, `connect-src 'self' ipc:
http://ipc.localhost`) hold as they are.
