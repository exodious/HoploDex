# HoploDex {{version}}

Which systems this release supports, and which package is for which, is in
[Supported platforms](https://github.com/exodious/HoploDex/blob/{{tag}}/README.md#supported-platforms).

## Install

**Windows:** run the setup `.exe`. It isn't signed with a paid certificate, so
Windows may say "Windows protected your PC": choose **More info**, then
**Run anyway**. A computer in S mode runs only Microsoft Store apps, so it has
to leave S mode first: **Settings › System › Activation**, then **Switch out
of S mode**, which opens the Microsoft Store. It is free, and can't be undone.

**macOS (Apple silicon):** open the `.dmg` and drag HoploDex to Applications.
It isn't notarized by Apple, so the first time you open it macOS says it can't
check it: choose **Done**, then in **System Settings › Privacy & Security**
find the message about HoploDex and choose **Open Anyway**.

**Linux, the Flatpak:** `flatpak install --user HoploDex-<version>-x86_64.flatpak`,
or open the file with your software centre. This adds Flathub if it isn't set
up and fetches the GNOME runtime. A newer version installs the same way, over
the old one, keeping everything. `flatpak update` updates the runtime but not
a HoploDex installed from a file.

**Linux, the `.deb`** (Ubuntu): `sudo apt install ./<the .deb>`.
**The `.rpm`** (Fedora): `sudo dnf install ./<the .rpm>`.

## Checksums

`SHA256SUMS` lists each file's SHA-256. In the folder you downloaded to,
`sha256sum --check --ignore-missing SHA256SUMS` checks the files you have.

The notices for the third-party components each package carries are in its
`NOTICES.txt` and in About HoploDex.
