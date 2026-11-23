# HoploDex {{version}}

Which systems this release supports, and which package is for which, is in
[Supported platforms](https://github.com/exodious/HoploDex/blob/{{tag}}/README.md#supported-platforms).

## Install

**Windows:** run `HoploDex-{{app_version}}-x86_64-setup.exe`. It isn't
signed with a paid certificate, so Windows may say "Windows protected your
PC": choose **More info**, then **Run anyway**. A computer in S mode runs only
Microsoft Store apps, so it has to leave S mode first: **Settings › System ›
Activation**, then **Switch out of S mode**, which opens the Microsoft Store.
It is free, and can't be undone.

**macOS (Apple silicon):** open `HoploDex-{{app_version}}-aarch64.dmg` and
drag HoploDex to Applications. It isn't notarized by Apple, so the first time
you open it macOS says it can't check it: choose **Done**, then in **System
Settings › Privacy & Security** find the message about HoploDex and choose
**Open Anyway**.

**Linux, the Flatpak:** `flatpak install --user HoploDex-{{app_version}}-x86_64.flatpak`,
or open the file with your software centre. This adds Flathub if it isn't set
up and fetches the GNOME runtime. A newer version installs the same way, over
the old one, keeping everything. `flatpak update` updates the runtime but not
a HoploDex installed from a file.
Each spreadsheet you import or folder you export to through the Flatpak's file
chooser stays open to HoploDex after it quits, until you take that back:
`flatpak permission-reset io.github.exodious.HoploDex`, then log out and in.

**Linux, the `.deb`** (Ubuntu): `sudo apt install ./HoploDex-{{app_version}}-x86_64.deb`.
**The `.rpm`** (Fedora): `sudo dnf install ./HoploDex-{{app_version}}-x86_64.rpm`.
Either installs the package `hoplodex` (`sudo apt remove hoplodex`,
`sudo dnf remove hoplodex`), which replaces 0.1.0's `hoplo-dex`.

## Checksums

`SHA256SUMS` lists each file's SHA-256. In the folder you downloaded to,
`sha256sum --check --ignore-missing SHA256SUMS` checks the files you have.

The notices for the third-party components each package carries are in its
`NOTICES.txt` and in About HoploDex.
