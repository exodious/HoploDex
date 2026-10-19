"""The Flatpak check's stand-in for the OpenURI portal (011 research.md §6, §8, T102).

A python-dbusmock template: `org.freedesktop.portal.Desktop` with the one
interface the app's portal route calls, `org.freedesktop.portal.OpenURI`. It
does what the real portal does with `OpenFile(parent_window, fd, options)`
that the check needs to see:

- reads what the descriptor is (the file it names, its access mode, its bytes)
  and appends one JSON line about it to the `log` parameter's file;
- appends the file's path to the `opened` parameter's file, the one line the
  `e2e` build's own opener writes (HOPLODEX_E2E_OPENED_LOG), so a journey reads
  both the same way;
- replies with the request object's path, built from the caller's unique name
  and `options["handle_token"]` as the portal does, and a moment later sends the
  request's `Response(0, {})` to the caller. The answer comes after the reply, as
  it does from the real portal, so the app's subscription (made first) is what
  receives it.

Run it with `python3 -m dbusmock --session -t scripts/flatpak/openuri_mock.py -p
'{"log": "...", "opened": "..."}'` on the check's private session bus only.
Never started with the developer's session bus: check.sh and
e2e/support/flatpak.ts pass the private bus address explicitly.
"""

import fcntl
import hashlib
import json
import os

import dbus
from gi.repository import GLib

BUS_NAME = "org.freedesktop.portal.Desktop"
MAIN_OBJ = "/org/freedesktop/portal/desktop"
MAIN_IFACE = "org.freedesktop.portal.OpenURI"
SYSTEM_BUS = False

REQUEST_IFACE = "org.freedesktop.portal.Request"
# How long after the reply the Response is sent.
RESPONSE_DELAY_MS = 50

_parameters = {}


def load(mock, parameters):
    _parameters.update(parameters or {})


def _plain(value):
    """A dbus value as plain JSON data."""
    if isinstance(value, dbus.Boolean):
        return bool(value)
    if isinstance(value, (dbus.Int16, dbus.Int32, dbus.Int64, dbus.UInt16, dbus.UInt32, dbus.UInt64, dbus.Byte)):
        return int(value)
    if isinstance(value, dbus.Double):
        return float(value)
    if isinstance(value, dbus.String):
        return str(value)
    if isinstance(value, dbus.Dictionary):
        return {str(k): _plain(v) for k, v in value.items()}
    if isinstance(value, (dbus.Array, list, tuple)):
        return [_plain(v) for v in value]
    return str(value)


def _append(name, line):
    target = _parameters.get(name)
    if not target:
        return
    with open(target, "a", encoding="utf-8") as out:
        out.write(line + "\n")


@dbus.service.method(
    MAIN_IFACE,
    in_signature="sha{sv}",
    out_signature="o",
    sender_keyword="sender",
)
def OpenFile(self, parent_window, fd, options, sender=None):
    number = fd.take() if hasattr(fd, "take") else int(fd)
    try:
        path = os.readlink(f"/proc/self/fd/{number}")
        flags = fcntl.fcntl(number, fcntl.F_GETFL)
        with os.fdopen(os.dup(number), "rb", closefd=True) as handle:
            handle.seek(0)
            content = handle.read()
        entry = {
            "path": path,
            "accessMode": flags & os.O_ACCMODE,  # 0 is read only
            "size": len(content),
            "sha256": hashlib.sha256(content).hexdigest(),
            "parentWindow": str(parent_window),
            "options": _plain(options),
        }
    finally:
        os.close(number)
    _append("log", json.dumps(entry, sort_keys=True))
    _append("opened", path)

    token = str(options.get("handle_token", "hoplodex")) if options else "hoplodex"
    handle_path = "/org/freedesktop/portal/desktop/request/{}/{}".format(
        str(sender).lstrip(":").replace(".", "_"), token
    )

    def respond():
        self.EmitSignalDetailed(
            REQUEST_IFACE,
            "Response",
            "ua{sv}",
            [dbus.UInt32(0), dbus.Dictionary({}, signature="sv")],
            {"path": handle_path, "destination": str(sender)},
        )
        return False

    GLib.timeout_add(RESPONSE_DELAY_MS, respond)
    return dbus.ObjectPath(handle_path)
