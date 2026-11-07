# HoploDex
Firearm Collection Inventory App

A local-only desktop app for keeping an inventory of a firearm collection,
built with Tauri 2: a Rust backend (`src-tauri/`) owns persistence,
encryption, and business logic; a React + TypeScript frontend (`src/`)
owns the UI.

## The troubleshooting log

While HoploDex is before version 1.0, it keeps a troubleshooting log on this
computer, up to 4 MiB and for up to 14 days. It is designed to leave out your
records, file names, passphrases and how many items you have. Nothing is sent
anywhere, and you can read it before you share it. You can turn it off in About.
From version 1.0 it is off unless you turn it on. A file over 14 days old is
deleted the next time HoploDex starts.

To gather a log for a problem you can repeat:

1. Open **About HoploDex**, and turn the log on if it is off.
2. Choose **Detailed until you quit**, and repeat the problem.
3. Choose **Show log file**, read it, and attach it to your report.

If the problem happens before you can reach About, start HoploDex with extra
detail for that one run:

| Where | The one line |
|---|---|
| Linux | `HOPLODEX_LOG=debug hoplodex` |
| Flatpak | `flatpak run --env=HOPLODEX_LOG=debug io.github.exodious.HoploDex` |
| macOS | `open --env HOPLODEX_LOG=debug -a HoploDex` |
| Windows (PowerShell) | `$env:HOPLODEX_LOG='debug'; & "<the folder HoploDex is installed in>\hoplodex.exe"` |

The file is `hoplodex.log` in the `logs` folder of HoploDex's cache:

| Where | The folder |
|---|---|
| Linux | `~/.cache/io.github.exodious.HoploDex/logs` |
| Flatpak | `~/.var/app/io.github.exodious.HoploDex/cache/io.github.exodious.HoploDex/logs` |
| macOS | `~/Library/Caches/io.github.exodious.HoploDex/logs` |
| Windows | `%LOCALAPPDATA%\io.github.exodious.HoploDex\logs` |

`HOPLODEX_LOG=off` writes nothing for that run, and `HOPLODEX_LOG=on` writes at the
usual level.

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
