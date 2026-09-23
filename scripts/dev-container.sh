#!/usr/bin/env bash
# Runs a command (default: an interactive shell) in the HoploDex development
# container, built from the repo's Dockerfile. See "Development container" in
# README.md.
#
#   scripts/dev-container.sh                      interactive shell in /workspace
#   scripts/dev-container.sh npm run test:e2e     run one command and exit
#   scripts/dev-container.sh --gui npm run tauri dev
#                                                 also show windows on your desktop
#   scripts/dev-container.sh --build              (re)build the image first
#
# Rootless podman, as the image's non-root `dev` user. --userns=keep-id maps
# your host user onto `dev`, so files written to the checkout stay yours.
#
# The checkout is bind-mounted at /workspace. node_modules and
# src-tauri/target are named volumes instead, per checkout, because the
# container's builds link against different system libraries than the host's
# and would otherwise overwrite each other. /home/dev is a named volume too,
# shared by all checkouts: shell history, the cargo crate cache, the keyring,
# and gh / Claude Code logins persist there.
#
# Environment:
#   CONTAINER_ENGINE   podman (default) or docker
#   HOPLODEX_IMAGE     image tag (default hoplodex-dev)
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
engine="${CONTAINER_ENGINE:-podman}"
image="${HOPLODEX_IMAGE:-hoplodex-dev}"

build=0
gui=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --build) build=1; shift ;;
    --gui) gui=1; shift ;;
    -h | --help)
      sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d; s/^# \{0,1\}//'
      exit 0
      ;;
    --) shift; break ;;
    *) break ;;
  esac
done
[[ $# -gt 0 ]] || set -- bash

if [[ $build -eq 1 ]] || ! "$engine" image inspect "$image" >/dev/null 2>&1; then
  "$engine" build -t "$image" "$repo"
fi

# Per-checkout volume names, so worktrees don't share build output.
checkout="$(printf '%s' "$repo" | sha256sum | cut -c1-8)"

args=(
  run --rm --init
  --hostname hoplodex-dev
  --security-opt label=disable
  --shm-size 1g
  -v "$repo:/workspace"
  -v "hoplodex-home:/home/dev"
  -v "hoplodex-$checkout-node-modules:/workspace/node_modules"
  -v "hoplodex-$checkout-target:/workspace/src-tauri/target"
  -w /workspace
)

if [[ "$engine" == podman ]]; then
  args+=(--userns=keep-id:uid=1000,gid=1000)
else
  # Docker has no keep-id; the image's dev user is uid 1000, which matches the
  # first user on most Linux installs.
  [[ "$(id -u)" == 1000 ]] || echo "warning: host uid $(id -u) isn't 1000; files written to the checkout will be owned by uid 1000" >&2
fi

[[ -t 0 && -t 1 ]] && args+=(-it)

# Your git identity (as the system config, so the container's own ~/.gitconfig
# stays writable), and your SSH agent for pushes.
[[ -f "$HOME/.gitconfig" ]] && args+=(-v "$HOME/.gitconfig:/etc/gitconfig:ro")
if [[ -n "${SSH_AUTH_SOCK:-}" && -S "$SSH_AUTH_SOCK" ]]; then
  args+=(-v "$SSH_AUTH_SOCK:/run/host/ssh-agent.sock" -e SSH_AUTH_SOCK=/run/host/ssh-agent.sock)
fi
for var in TERM COLORTERM GH_TOKEN GITHUB_TOKEN ANTHROPIC_API_KEY; do
  [[ -n "${!var:-}" ]] && args+=(-e "$var")
done

if [[ $gui -eq 1 ]]; then
  if [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
    socket="$WAYLAND_DISPLAY"
    [[ "$socket" == /* ]] || socket="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/$socket"
    args+=(-v "$socket:/run/host/wayland-0" -e WAYLAND_DISPLAY=/run/host/wayland-0)
  fi
  if [[ -n "${DISPLAY:-}" ]]; then
    args+=(-v /tmp/.X11-unix:/tmp/.X11-unix:ro -e DISPLAY)
    [[ -n "${XAUTHORITY:-}" && -f "$XAUTHORITY" ]] &&
      args+=(-v "$XAUTHORITY:/run/host/Xauthority:ro" -e XAUTHORITY=/run/host/Xauthority)
  fi
  [[ -e /dev/dri ]] && args+=(--device /dev/dri)
  if [[ -z "${WAYLAND_DISPLAY:-}" && -z "${DISPLAY:-}" ]]; then
    echo "error: --gui needs WAYLAND_DISPLAY or DISPLAY set" >&2
    exit 1
  fi
fi

exec "$engine" "${args[@]}" "$image" "$@"
