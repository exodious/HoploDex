# SC-007 evidence: the Flatpak check does not touch the developer's data

Recorded 2026-10-19 on the Linux host (T036; `011 US5-3`, `011 SC-007`) for the
pull request. Nothing here opened a database, a keyring or any file under
`~/.var/app`: the host commands are read-only listings.

## 1. The host before and after T035's container runs

T035 ran `scripts/dev-container.sh --flatpak bash -c 'npm run build && npm run
test:flatpak'` four times (the last of them passing). The listings were taken
at 13:22 before the first run and at 13:41 after the last.

| Command on the host | Before | After |
|---|---|---|
| `flatpak --user list --app` | `flatpak` is not installed on this host: command not found | the same |
| `flatpak --system list --app` | command not found | the same |
| `ls ~/.var/app` | `No such file or directory` | the same |

The two outputs are byte-for-byte equal (`diff` reported no difference). The
host has no Flatpak at all, so a host installation could not have changed; the
guard (`scripts/flatpak/guard.mjs`) is what keeps it so on a host that has one.
`~/.local/share/flatpak/db` on the host dates from 2026-09-17 and was not
modified. `/var/lib/flatpak` does not exist.

Inside the container, the same runs ended with the check variant uninstalled
and `flatpak list --user --app` equal to its start (`none`), which
`check.sh` fails the run for otherwise.

## 2. What a `--flatpak` container mounts

`podman inspect` of a running `scripts/dev-container.sh --flatpak sleep 45`
container (`Privileged=false`, `Devices=[]`, `User=dev`, `SecurityOpt=[label=disable]`
plus the `unmask=/proc/*` that `--flatpak` adds):

| Type | Host source | Container path |
|---|---|---|
| volume | podman volume `hoplodex-f4961d8f-node-modules` | `/workspace/node_modules` |
| volume | podman volume `hoplodex-f4961d8f-target` | `/workspace/src-tauri/target` |
| volume | podman volume `hoplodex-f4961d8f-flatpak-state` | `/var/lib/hoplodex-flatpak-state` |
| volume | podman volume `hoplodex-home` | `/home/dev` |
| bind | `/home/claude/HoploDex` (the checkout) | `/workspace` |
| tmpfs | none | `/mnt/hoplodex-check` |

Against the list the task names:

- the host's home folder: not mounted. Only the checkout inside it is, and
  `/home/dev` is a podman volume, not `/home/claude`;
- `~/Documents`, `~/.config` (so no `machine.json` and no recent-databases
  list), `~/.var/app`: not mounted;
- `/run/user/<uid>` (`/run/user/1001`, the host's session bus, system-user
  runtime folder and keyring socket): not mounted. The check starts its own
  private session bus and private system bus in the container;
- the host's Flatpak installations (`~/.local/share/flatpak`,
  `/var/lib/flatpak`): not mounted. The container's installation is
  `/home/dev/.local/share/flatpak` in the `hoplodex-home` volume.

The container's keyring is the `hoplodex-home` volume's, and the check's stand-in
Secret Service answers on its private session bus, so no entry in the host's
keyring is reachable.
