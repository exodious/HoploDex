# Development container (Linux, recommended)

_Part of the [development docs](../../DEVELOPMENT.md)._

The repo's `Dockerfile` (Debian trixie) has everything in
[Prerequisites](prerequisites.md) already installed: Rust, Node 24 LTS with npm 12, the Tauri/WebKitGTK and SQLCipher
build dependencies, `cargo-deny`, `cargo-nextest`, Xvfb,
gnome-keyring, the GitHub CLI, Claude Code, Spec Kit's `specify`, and
`python3-gi` for GTK drag-and-drop tests. It's built for rootless [podman](https://podman.io) and
runs as a non-root `dev` user. The only thing to install on the host is
podman.

```bash
scripts/dev-container.sh                      # shell in /workspace (builds the image on first use)
scripts/dev-container.sh npm test             # or run one command and exit
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'   # several commands
scripts/dev-container.sh --build              # rebuild the image, e.g. after pulling Dockerfile changes
scripts/dev-container.sh --reset-volumes      # start over with empty node_modules and target volumes
scripts/dev-container.sh --gui npm run tauri dev   # show the app's window on your desktop
scripts/dev-container.sh --git-config --ssh-agent  # a shell you can commit and push from
```

Every command in these documents works unchanged inside the container.
Inside it, the working directory is `/workspace` and the hostname is
`hoplodex-dev`. Your checkout is bind-mounted at `/workspace`, and
`--userns=keep-id` maps your host user onto `dev`, so files the container
writes stay owned by you. Some things are kept in named volumes rather than
in the checkout, so they don't collide with host builds:

- `node_modules` and `src-tauri/target`, one pair per checkout. The container
  links against its own system libraries, so its builds can't share these
  with the host. On first run the container fills `node_modules` with
  `npm ci`, and the first cargo build starts from scratch, so the first run
  of anything is slow. A no-mistakes review run gets a new worktree under
  `~/.no-mistakes/worktrees/` (or `$NM_HOME`) each time, so its pair is
  keyed by the run's branch instead, read from the daemon's run record:
  reruns of a PR reuse a warm build, and different PRs never share one. If
  the branch can't be read (no `sqlite3` on the host, or no record), the run
  gets anonymous volumes, deleted when its container exits. A reviewed
  branch's `hoplodex-review-<hash>-*` volumes stay until you remove them
  (`podman volume ls -q | grep '^hoplodex-review-' | xargs -r podman volume rm`).
- `/home/dev`, shared by all checkouts: shell history, the cargo crate cache,
  the keyring, and `gh` / `claude` logins. Log in once with `gh auth login` and
  `claude` inside the container.

Nothing from your host's identity is shared by default: no git config, SSH
agent or tokens. To commit and push from inside, pass `--git-config` to mount
your `~/.gitconfig` read-only (or `--git-config=FILE` for another one) and
`--ssh-agent` to forward your SSH agent socket. To use tokens instead of
logging in, export them on the host and pass `--gh-token` (`GH_TOKEN` /
`GITHUB_TOKEN`) or `--anthropic-api-key` (`ANTHROPIC_API_KEY`). Options go
before the command. Tests and screenshots run
headless on Xvfb and need no display. `--gui` forwards your Wayland or X11
socket and `/dev/dri`, for `tauri dev` and human testing. `podman volume ls |
grep hoplodex` lists the volumes. `--reset-volumes` deletes this checkout's
`node_modules` and `target` volumes before starting, so they're rebuilt from
scratch; `--reset-volumes=all` also deletes the shared home volume, which
logs you out of `gh` and `claude`.
`CONTAINER_ENGINE=docker` works too. Docker has no `keep-id`, though, so files
end up owned by uid 1000, which is fine if that's your uid.

On macOS the container runs in podman's Linux VM, which can't see XQuartz's
Unix socket, so `--gui` connects to XQuartz over TCP instead and renders in
software, with a 24 px pointer (XQuartz gives clients no size, so GTK would pick
48 px on a 4K screen). One-time setup: install XQuartz, tick "Allow connections from
network clients" in its Settings > Security (or `defaults write
org.xquartz.X11 nolisten_tcp -bool false`), quit and reopen it, and run
`xhost +localhost` (the VM's connection arrives as a local one, so this admits
only your own machine). The script checks that XQuartz is listening and says
what to change if it isn't.

The container has none of the host's app data, so nothing run inside it can
reach your real collection (see [Test isolation](test-isolation.md)).

To set up a host directly instead (or on macOS/Windows), install the
[prerequisites](prerequisites.md).
