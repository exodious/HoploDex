#!/usr/bin/env python3
"""Checks FR-035 / SC-010 on the real app: a decrypted copy of an opened
document must be gone once the app exits, however it is asked to quit.

The WebDriver E2E suite can't check this. WebKitWebDriver tears the app down
when a session ends without letting it run its exit handler, so the wdio specs
verify the crash/relaunch half (the startup sweep) and this script verifies the
exit half. It launches the app on a virtual display with scratch data/cache
dirs (never the real ones), plants a file where `open_document` writes its
temporary copies, quits the app, and looks at what is left.

Needs Xvfb, libX11 and python3 only. Build the E2E binary first:

    npm run build && cargo build --release --features custom-protocol,mock-keyring \
        --manifest-path src-tauri/Cargo.toml
    xvfb-run -a python3 e2e/scripts/quit-cleanup.py
"""

import ctypes
import ctypes.util
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from ctypes import POINTER, Structure, Union, byref, c_char_p, c_int, c_long, c_uint, c_ulong, c_void_p

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BINARY = os.environ.get("HOPLODEX_BIN", os.path.join(ROOT, "src-tauri", "target", "release", "hoplodex"))
WINDOW_TITLE = "HoploDex"

x11 = ctypes.cdll.LoadLibrary(ctypes.util.find_library("X11"))
x11.XOpenDisplay.restype = c_void_p
x11.XOpenDisplay.argtypes = [c_char_p]
x11.XInternAtom.restype = c_ulong
x11.XInternAtom.argtypes = [c_void_p, c_char_p, c_int]
x11.XDefaultRootWindow.restype = c_ulong
x11.XDefaultRootWindow.argtypes = [c_void_p]
x11.XQueryTree.argtypes = [c_void_p, c_ulong, POINTER(c_ulong), POINTER(c_ulong), POINTER(POINTER(c_ulong)), POINTER(c_uint)]
x11.XFetchName.argtypes = [c_void_p, c_ulong, POINTER(c_char_p)]
x11.XFlush.argtypes = [c_void_p]


class XClientMessageEvent(Structure):
    _fields_ = [("type", c_int), ("serial", c_ulong), ("send_event", c_int), ("display", c_void_p),
                ("window", c_ulong), ("message_type", c_ulong), ("format", c_int), ("data", c_long * 5)]


class XEvent(Union):
    _fields_ = [("type", c_int), ("xclient", XClientMessageEvent), ("pad", c_long * 24)]


x11.XSendEvent.argtypes = [c_void_p, c_ulong, c_int, c_long, POINTER(XEvent)]


def windows_named(dpy, parent, title, depth=0):
    found = []
    root, up = c_ulong(), c_ulong()
    kids, count = POINTER(c_ulong)(), c_uint()
    if not x11.XQueryTree(dpy, parent, byref(root), byref(up), byref(kids), byref(count)):
        return found
    for i in range(count.value):
        win = kids[i]
        name = c_char_p()
        if x11.XFetchName(dpy, win, byref(name)) and name.value and name.value.decode(errors="ignore") == title:
            found.append(win)
        if depth < 2:
            found += windows_named(dpy, win, title, depth + 1)
    return found


def close_window(dpy, windows):
    """What clicking the window's close button does: ask politely."""
    event = XEvent()
    event.xclient.type = 33  # ClientMessage
    event.xclient.format = 32
    event.xclient.message_type = x11.XInternAtom(dpy, b"WM_PROTOCOLS", False)
    event.xclient.data[0] = x11.XInternAtom(dpy, b"WM_DELETE_WINDOW", False)
    for win in windows:
        event.xclient.window = win
        x11.XSendEvent(dpy, win, False, 0, byref(event))
    x11.XFlush(dpy)


def run(how):
    scratch = tempfile.mkdtemp(prefix="hoplodex-quit-")
    env = dict(os.environ, GDK_BACKEND="x11", XDG_DATA_HOME=scratch + "/data",
               XDG_CACHE_HOME=scratch + "/cache", XDG_CONFIG_HOME=scratch + "/config")
    env.pop("WAYLAND_DISPLAY", None)
    opened = scratch + "/cache/com.hoplodex.inventory/opened-documents"
    planted = opened + "/7/receipt.pdf"
    app = subprocess.Popen([BINARY], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        dpy = x11.XOpenDisplay(None)
        root = x11.XDefaultRootWindow(dpy)
        deadline = time.time() + 30
        windows = []
        while time.time() < deadline and not windows:
            windows = windows_named(dpy, root, WINDOW_TITLE)
            time.sleep(0.5)
        if not windows:
            return False, "the app's window never appeared"
        time.sleep(3)  # let startup (and its sweep of old copies) finish first

        os.makedirs(os.path.dirname(planted))
        with open(planted, "w") as f:
            f.write("%PDF-1.4 decrypted copy")

        if how == "close":
            close_window(dpy, windows)
        else:
            app.send_signal({"term": signal.SIGTERM, "hup": signal.SIGHUP, "int": signal.SIGINT, "kill": signal.SIGKILL}[how])
        try:
            app.wait(timeout=20)
        except subprocess.TimeoutExpired:
            return False, "the app didn't exit within 20s"
        time.sleep(0.5)
        left = os.path.exists(planted)
        if how == "kill":
            # A killed process can't clean up; the next launch's sweep does.
            return left, "left behind for the next launch to sweep" if left else "was somehow cleaned up"
        return not left, "removed on exit" if not left else "still on disk after exit"
    finally:
        if app.poll() is None:
            app.kill()
        shutil.rmtree(scratch, ignore_errors=True)


if __name__ == "__main__":
    if not os.path.exists(BINARY):
        sys.exit(f"build the E2E binary first: {BINARY} not found")
    failures = 0
    for how, label in [("close", "window closed"), ("term", "SIGTERM"), ("hup", "SIGHUP"), ("int", "SIGINT"), ("kill", "SIGKILL (control)")]:
        ok, detail = run(how)
        print(f"{'PASS' if ok else 'FAIL'}  {label}: {detail}")
        failures += not ok
    sys.exit(1 if failures else 0)
