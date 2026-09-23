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
#   scripts/dev-container.sh --reset-volumes      start from empty build volumes
#   scripts/dev-container.sh --git-config --ssh-agent
#                                                 commit and push from inside
#
# Options (before the command):
#   --build              (re)build the image first
#   --reset-volumes[=all]
#                        delete this checkout's node_modules and target volumes
#                        first; =all also deletes the shared /home/dev volume
#                        (logins, caches, history). Bind mounts are untouched.
#   --gui                forward your Wayland/X11 display and /dev/dri
#   --git-config[=FILE]  mount your git config read-only (default ~/.gitconfig,
#                        else ~/.config/git/config)
#   --ssh-agent          forward your SSH agent socket ($SSH_AUTH_SOCK)
#   --gh-token           pass through GH_TOKEN and/or GITHUB_TOKEN, for gh
#   --anthropic-api-key  pass through ANTHROPIC_API_KEY, for Claude Code
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
reset_volumes=
gui=0
git_config=
ssh_agent=0
env_vars=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --build) build=1; shift ;;
    --reset-volumes) reset_volumes=checkout; shift ;;
    --reset-volumes=all) reset_volumes=all; shift ;;
    --reset-volumes=*)
      echo "error: --reset-volumes takes no value or =all, not '${1#*=}'" >&2
      exit 1
      ;;
    --gui) gui=1; shift ;;
    --git-config)
      git_config="$HOME/.gitconfig"
      [[ -f "$git_config" ]] || git_config="${XDG_CONFIG_HOME:-$HOME/.config}/git/config"
      shift
      ;;
    --git-config=*) git_config="${1#*=}"; shift ;;
    --ssh-agent) ssh_agent=1; shift ;;
    --gh-token)
      [[ -n "${GH_TOKEN:-}" ]] && env_vars+=(GH_TOKEN)
      [[ -n "${GITHUB_TOKEN:-}" ]] && env_vars+=(GITHUB_TOKEN)
      if [[ -z "${GH_TOKEN:-}" && -z "${GITHUB_TOKEN:-}" ]]; then
        echo "error: --gh-token needs GH_TOKEN or GITHUB_TOKEN set" >&2
        exit 1
      fi
      shift
      ;;
    --anthropic-api-key)
      if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
        echo "error: --anthropic-api-key needs ANTHROPIC_API_KEY set" >&2
        exit 1
      fi
      env_vars+=(ANTHROPIC_API_KEY)
      shift
      ;;
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
home_volume=hoplodex-home
node_modules_volume="hoplodex-$checkout-node-modules"
target_volume="hoplodex-$checkout-target"

# Deleted volumes are recreated empty by the run below. Removal fails, and so
# does the script, if a running container still uses one.
if [[ -n "$reset_volumes" ]]; then
  volumes=("$node_modules_volume" "$target_volume")
  [[ "$reset_volumes" == all ]] && volumes+=("$home_volume")
  for volume in "${volumes[@]}"; do
    if "$engine" volume inspect "$volume" >/dev/null 2>&1; then
      "$engine" volume rm "$volume" >/dev/null
      echo "removed volume $volume" >&2
    fi
  done
fi

args=(
  run --rm --init
  --hostname hoplodex-dev
  --security-opt label=disable
  --shm-size 1g
  -v "$repo:/workspace"
  -v "$home_volume:/home/dev"
  -v "$node_modules_volume:/workspace/node_modules"
  -v "$target_volume:/workspace/src-tauri/target"
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

# Your git config, only with --git-config (as the system config, so the
# container's own ~/.gitconfig stays writable), and your SSH agent for pushes,
# only with --ssh-agent.
if [[ -n "$git_config" ]]; then
  if [[ ! -f "$git_config" ]]; then
    echo "error: --git-config: no git config at $git_config" >&2
    exit 1
  fi
  args+=(-v "$(realpath "$git_config"):/etc/gitconfig:ro")
fi
if [[ $ssh_agent -eq 1 ]]; then
  if [[ -z "${SSH_AUTH_SOCK:-}" || ! -S "$SSH_AUTH_SOCK" ]]; then
    echo "error: --ssh-agent needs SSH_AUTH_SOCK set to a running agent's socket" >&2
    exit 1
  fi
  args+=(-v "$SSH_AUTH_SOCK:/run/host/ssh-agent.sock" -e SSH_AUTH_SOCK=/run/host/ssh-agent.sock)
fi
# Tokens, only with --gh-token / --anthropic-api-key. Passed by name, so the
# values stay off the command line.
for var in "${env_vars[@]}"; do
  args+=(-e "$var")
done
for var in TERM COLORTERM; do
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
