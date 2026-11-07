# Architecture

Read only the part your change touches.

| To change | Read | Old section name |
|---|---|---|
| The boundary between frontend and backend, or an error code | [IPC boundary](architecture/ipc.md) | "IPC boundary" |
| The frontend, or how a file drop arrives | [Frontend](architecture/frontend.md) | "Frontend", "File drops" |

**Backend layering** (`src-tauri/src/`):

| To change | Read | Old section name |
|---|---|---|
| `commands/`, `session/`: a command, or what one open owns | [Commands and the session](architecture/commands-and-session.md) | "Backend layering" |
| `models/`, `services/`: types, validation, domain logic, the passphrase dialog, keyring, sandbox and Flatpak reach, helpers | [Models and services](architecture/models-and-services.md) | "Backend layering" |
| `services/preview/`: showing a document | [Document preview](architecture/preview.md) | "Backend layering" |
| `db/`: creating and opening databases, migrations, the cipher | [The database](architecture/database.md) | "Backend layering" |
| `main.rs`, `app_dirs.rs`, `platform/`, `window_controls/`: startup, directories, OS events, crash handling | [Startup and the platform](architecture/startup-and-platform.md) | "Backend layering" |
| `scripts/licenses/`, `licenses/`: the third-party notices | [Licenses](architecture/licenses.md) | "Backend layering" |
| `flatpak/`, `scripts/flatpak/`: what builds the Flatpak | [Flatpak](architecture/flatpak.md) | "Backend layering" |
