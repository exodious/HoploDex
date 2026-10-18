# HoploDex
Firearm Collection Inventory App

A local-only desktop app for keeping an inventory of a firearm collection,
built with Tauri 2: a Rust backend (`src-tauri/`) owns persistence,
encryption, and business logic; a React + TypeScript frontend (`src/`)
owns the UI.

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for the development container,
prerequisites, and how to build, test, lint, take screenshots and run the
app against a seeded human-testing collection.

## License

Copyright (C) 2026 exodious and the HoploDex contributors

HoploDex is free software: you can redistribute it and/or modify it under the
terms of the GNU General Public License as published by the Free Software
Foundation, version 3 only. It is distributed in the hope that it will be
useful, but WITHOUT ANY WARRANTY, to the extent permitted by law; without even
the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
See the GNU General Public License in [LICENSE](LICENSE) for more details.

Additional permission under GNU GPL version 3 section 7

If you modify this Program, or any covered work, by linking or combining it
with Microsoft's WebView2 loader (WebView2Loader or WebView2LoaderStatic,
from the Microsoft WebView2 SDK), or a modified version of that library,
containing parts covered by the terms of the Microsoft WebView2 SDK's
license, the licensors of this Program grant you additional permission to
convey the resulting work.

The copy of this permission that the build's notices read is
[licenses/additional-permission.txt](licenses/additional-permission.txt).

Contributions are accepted under this license, with this additional
permission.

The notices for the third-party components in each build are in that build's
`NOTICES.txt` and in About HoploDex.
