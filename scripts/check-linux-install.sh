#!/bin/sh
# Installs a built .deb or .rpm on a clean container image with only the
# package manager's dependency resolution (#208), then fails if:
#   - `ldd /usr/bin/hoplodex` reports a library `not found`, or xdg-open
#     (which opens a document in another app outside the Flatpak) is missing;
#   - a library the binary links (readelf -d's NEEDED) isn't covered by what
#     the package declares: in the .rpm, a `<soname>()(64bit)` requirement; in
#     the .deb, a Depends naming the package that installed it. So a new link
#     dependency can't slip in undeclared and work only where the base system
#     happens to have it.
# Weak dependencies and Recommends stay off, so only what's declared counts.
#
#   scripts/check-linux-install.sh IMAGE PACKAGE
#
# e.g. fedora:45 with the .rpm, ubuntu:24.04 with the .deb. Runs the image
# with docker, or podman where docker isn't installed, and calls itself
# inside it with --inside.
set -eu

if [ "${1:-}" != --inside ]; then
  [ $# -eq 2 ] || { echo "usage: $0 IMAGE PACKAGE" >&2; exit 2; }
  image=$1
  pkg=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
  self=$(cd "$(dirname "$0")" && pwd)/$(basename "$0")
  engine=$(command -v docker >/dev/null && echo docker || echo podman)
  exec "$engine" run --rm \
    -v "$pkg:/check/$(basename "$pkg"):ro" -v "$self:/check/check.sh:ro" \
    "$image" sh /check/check.sh --inside "/check/$(basename "$pkg")"
fi

pkg=$2
bin=/usr/bin/hoplodex
case $pkg in
  *.rpm)
    dnf install -y -q --setopt=install_weak_deps=False "$pkg"
    ;;
  *.deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends "$pkg" >/dev/null
    ;;
  *) echo "not a .deb or .rpm: $pkg" >&2; exit 2 ;;
esac

. /etc/os-release
echo "Installed $(basename "$pkg") on $PRETTY_NAME"
problems=0

if ldd "$bin" | grep 'not found'; then
  echo "FAIL: libraries above are missing after installing the package" >&2
  problems=1
fi
if ! command -v xdg-open >/dev/null; then
  echo "FAIL: xdg-open is missing (xdg-utils)" >&2
  problems=1
fi

# readelf comes from binutils, installed only now so it can't satisfy anything above.
case $pkg in
  *.rpm) dnf install -y -q binutils ;;
  *.deb) apt-get install -y -qq --no-install-recommends binutils >/dev/null ;;
esac
needed=$(readelf -d "$bin" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p')
[ -n "$needed" ] || { echo "FAIL: no NEEDED entries read from $bin" >&2; exit 1; }

case $pkg in
  *.rpm) declared=$(rpm -qpR "$pkg") ;;
  # Package names only: no version floors, each alternative of an `a | b` its own name.
  *.deb) declared=$(dpkg-deb -f "$pkg" Depends | tr ',|' '\n\n' | sed 's/(.*)//; s/[[:space:]]//g') ;;
esac
for lib in $needed; do
  case $pkg in
    *.rpm) echo "$declared" | grep -Fqx "$lib()(64bit)" && continue ;;
    # The installed packages that own the library, e.g. `libc6:amd64: /usr/lib/x86_64-linux-gnu/libc.so.6`.
    *.deb)
      owners=$(dpkg -S "*/$lib" 2>/dev/null | sed 's/: .*//; s/:[a-z0-9]*//g' | tr ',' '\n' | sed 's/[[:space:]]//g')
      for o in $owners; do echo "$declared" | grep -Fqx "$o" && continue 2; done
      ;;
  esac
  echo "FAIL: $lib is linked but not declared" >&2
  problems=1
done

[ $problems -eq 0 ] && echo "OK: every library resolves, and each of the $(echo "$needed" | wc -l) NEEDED is declared"
exit $problems
