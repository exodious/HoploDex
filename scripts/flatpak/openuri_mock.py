"""The Flatpak check's stand-in for the desktop portals (011 research.md §6, §7, §8, §16).

A python-dbusmock template: `org.freedesktop.portal.Desktop` with the two
interfaces the app's portal routes call, `org.freedesktop.portal.OpenURI` and
`org.freedesktop.portal.Secret`. The Secret portal is the one the app keeps its
saved passphrases' key from (`RetrieveSecret(fd, options)`), served only when the
`secret` parameter is given; without it the stand-in has no Secret interface, as a
desktop with no Secret portal backend (011 US2-5). It does what the real portal does
with `OpenFile(parent_window, fd, options)` that the check needs to see:

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

`RetrieveSecret(fd, options)` writes the `secret` parameter (a fixed key for the run,
as text) to the descriptor, closes it, replies with the request object's path as
above and sends `Response(0, {})`, as ashpd's `secret::retrieve()` waits for. It never
reads or writes a keyring itself: the app's file keyring is encrypted with the key, and
the check reads it back with the same key (`src-tauri/examples/flatpak_keyring_read.rs`).

Run it with `python3 -m dbusmock --session -t scripts/flatpak/openuri_mock.py -p
'{"log": "...", "opened": "...", "secret": "..."}'` on the check's private session bus only.
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

SECRET_IFACE = "org.freedesktop.portal.Secret"
REQUEST_IFACE = "org.freedesktop.portal.Request"
# How long after the reply the Response is sent.
RESPONSE_DELAY_MS = 50

_parameters = {}


def load(mock, parameters):
    _parameters.update(parameters or {})
    if _parameters.get("secret"):
        # xdg-desktop-portal exports the Secret interface only when a backend
        # implements it; the app's availability probe reads this property.
        mock.AddProperty(SECRET_IFACE, "version", dbus.UInt32(1))


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


def _request_path(sender, options):
    token = str(options.get("handle_token", "hoplodex")) if options else "hoplodex"
    return "/org/freedesktop/portal/desktop/request/{}/{}".format(
        str(sender).lstrip(":").replace(".", "_"), token
    )


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

    handle_path = _request_path(sender, options)

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


@dbus.service.method(
    SECRET_IFACE,
    in_signature="ha{sv}",
    out_signature="o",
    sender_keyword="sender",
)
def RetrieveSecret(self, fd, options, sender=None):
    number = fd.take() if hasattr(fd, "take") else int(fd)
    try:
        secret = _parameters.get("secret")
        if not secret:
            raise dbus.exceptions.DBusException(
                "this stand-in has no Secret portal backend",
                name="org.freedesktop.DBus.Error.UnknownMethod",
            )
        os.write(number, str(secret).encode("utf-8"))
    finally:
        os.close(number)
    handle_path = _request_path(sender, options)

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
