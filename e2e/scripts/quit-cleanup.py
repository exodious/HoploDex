#!/usr/bin/env python3
"""Checks FR-035 / SC-010 on the real app: a decrypted copy of an opened
document must be gone once the app exits, however it is asked to quit.

The WebDriver E2E suite doesn't check this. Its harness kills the app with
SIGKILL (e2e/support/app.ts) without letting it run its exit handler, so the
wdio specs verify the crash/relaunch half (the startup sweep) and this script
verifies the exit half. It launches the app with scratch config, cache,
documents and (on macOS) home directories, never the real ones, plants a file
where `open_document` writes its temporary copies, quits the app, and looks at
what is left.

Linux and macOS; quit-cleanup.ps1 is its Windows counterpart (#27). Build the
E2E binary first:

    npm run build && cargo build --profile e2e --features custom-protocol,e2e \\
        --manifest-path src-tauri/Cargo.toml

Linux, on a virtual display (needs Xvfb, libX11 and python3 only):

    xvfb-run -a python3 e2e/scripts/quit-cleanup.py

macOS, from a logged-in desktop session, since an app started over SSH gets
no window. It presses the window's close button through the Accessibility API
and posts Cmd+Q to the app, so whatever runs it needs Accessibility (in a tart
VM, Terminal has it: `scripts/tart-vm.sh gui python3 e2e/scripts/quit-cleanup.py`).

    python3 e2e/scripts/quit-cleanup.py
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
import urllib.request
from ctypes import POINTER, Structure, Union, byref, c_char_p, c_int, c_long, c_uint, c_ulong, c_void_p

LINUX = sys.platform.startswith("linux")
MACOS = sys.platform == "darwin"

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BINARY = os.environ.get("HOPLODEX_BIN", os.path.join(ROOT, "src-tauri", "target", "e2e", "hoplodex"))
WINDOW_TITLE = "HoploDex"
IDENTIFIER = "io.github.exodious.HoploDex"
# The embedded WebDriver server answers once the app is up. The port is above
# the E2E workers' (4445 and up), so a suite running alongside doesn't collide.
WEBDRIVER_PORT = 4544


class X11:
    """Finds the app's window on $DISPLAY and closes it as a window manager
    would."""

    class ClientMessage(Structure):
        _fields_ = [("type", c_int), ("serial", c_ulong), ("send_event", c_int), ("display", c_void_p),
                    ("window", c_ulong), ("message_type", c_ulong), ("format", c_int), ("data", c_long * 5)]

    class Event(Union):
        pass

    Event._fields_ = [("type", c_int), ("xclient", ClientMessage), ("pad", c_long * 24)]

    def __init__(self):
        x11 = ctypes.cdll.LoadLibrary(ctypes.util.find_library("X11"))
        x11.XOpenDisplay.restype = c_void_p
        x11.XOpenDisplay.argtypes = [c_char_p]
        x11.XInternAtom.restype = c_ulong
        x11.XInternAtom.argtypes = [c_void_p, c_char_p, c_int]
        x11.XDefaultRootWindow.restype = c_ulong
        x11.XDefaultRootWindow.argtypes = [c_void_p]
        x11.XQueryTree.argtypes = [c_void_p, c_ulong, POINTER(c_ulong), POINTER(c_ulong),
                                   POINTER(POINTER(c_ulong)), POINTER(c_uint)]
        x11.XFetchName.argtypes = [c_void_p, c_ulong, POINTER(c_char_p)]
        x11.XFlush.argtypes = [c_void_p]
        x11.XSendEvent.argtypes = [c_void_p, c_ulong, c_int, c_long, POINTER(X11.Event)]
        self.x11 = x11
        self.dpy = x11.XOpenDisplay(None)
        if not self.dpy:
            sys.exit("no X display: run under xvfb-run -a")

    def windows_named(self, parent, title, depth=0):
        found = []
        root, up = c_ulong(), c_ulong()
        kids, count = POINTER(c_ulong)(), c_uint()
        if not self.x11.XQueryTree(self.dpy, parent, byref(root), byref(up), byref(kids), byref(count)):
            return found
        for i in range(count.value):
            win = kids[i]
            name = c_char_p()
            if (self.x11.XFetchName(self.dpy, win, byref(name)) and name.value
                    and name.value.decode(errors="ignore") == title):
                found.append(win)
            if depth < 2:
                found += self.windows_named(win, title, depth + 1)
        return found

    def has_window(self, _app):
        return bool(self.windows_named(self.x11.XDefaultRootWindow(self.dpy), WINDOW_TITLE))

    def close(self, _app):
        """What clicking the window's close button does: ask politely."""
        windows = self.windows_named(self.x11.XDefaultRootWindow(self.dpy), WINDOW_TITLE)
        event = X11.Event()
        event.xclient.type = 33  # ClientMessage
        event.xclient.format = 32
        event.xclient.message_type = self.x11.XInternAtom(self.dpy, b"WM_PROTOCOLS", False)
        event.xclient.data[0] = self.x11.XInternAtom(self.dpy, b"WM_DELETE_WINDOW", False)
        for win in windows:
            event.xclient.window = win
            self.x11.XSendEvent(self.dpy, win, False, 0, byref(event))
        self.x11.XFlush(self.dpy)


class MacOS:
    """Finds the app's window and presses its close button through the
    Accessibility API, or posts Cmd+Q to the app, as a person would."""

    UTF8 = 0x08000100  # kCFStringEncodingUTF8
    COMMAND = 0x100000  # kCGEventFlagMaskCommand
    KEY_Q = 12  # kVK_ANSI_Q

    def __init__(self):
        cf = ctypes.cdll.LoadLibrary("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
        ax = ctypes.cdll.LoadLibrary(
            "/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices")
        cf.CFStringCreateWithCString.restype = c_void_p
        cf.CFStringCreateWithCString.argtypes = [c_void_p, c_char_p, c_uint]
        cf.CFArrayGetCount.restype = c_long
        cf.CFArrayGetCount.argtypes = [c_void_p]
        cf.CFArrayGetValueAtIndex.restype = c_void_p
        cf.CFArrayGetValueAtIndex.argtypes = [c_void_p, c_long]
        cf.CFRelease.argtypes = [c_void_p]
        ax.AXIsProcessTrusted.restype = ctypes.c_bool
        ax.AXUIElementCreateApplication.restype = c_void_p
        ax.AXUIElementCreateApplication.argtypes = [c_int]
        ax.AXUIElementCopyAttributeValue.restype = c_int
        ax.AXUIElementCopyAttributeValue.argtypes = [c_void_p, c_void_p, POINTER(c_void_p)]
        ax.AXUIElementPerformAction.restype = c_int
        ax.AXUIElementPerformAction.argtypes = [c_void_p, c_void_p]
        ax.CGEventCreateKeyboardEvent.restype = c_void_p
        ax.CGEventCreateKeyboardEvent.argtypes = [c_void_p, ctypes.c_uint16, ctypes.c_bool]
        ax.CGEventSetFlags.argtypes = [c_void_p, ctypes.c_uint64]
        ax.CGEventPostToPid.argtypes = [c_int, c_void_p]
        self.cf, self.ax = cf, ax
        if not ax.AXIsProcessTrusted():
            sys.exit("this needs Accessibility: allow the program running it in System Settings > "
                     "Privacy & Security > Accessibility")

    def string(self, text):
        return self.cf.CFStringCreateWithCString(None, text.encode(), self.UTF8)

    def attribute(self, element, name):
        """An element's attribute, which the caller releases, or None."""
        value, key = c_void_p(), self.string(name)
        error = self.ax.AXUIElementCopyAttributeValue(element, key, byref(value))
        self.cf.CFRelease(key)
        return value.value if error == 0 else None

    def close_buttons(self, app):
        """The close buttons of the app's windows; release the windows list."""
        element = self.ax.AXUIElementCreateApplication(app.pid)
        try:
            windows = self.attribute(element, "AXWindows")
            if not windows:
                return None, []
            buttons = [self.cf.CFArrayGetValueAtIndex(windows, i) for i in range(self.cf.CFArrayGetCount(windows))]
            return windows, [b for b in (self.attribute(w, "AXCloseButton") for w in buttons) if b]
        finally:
            self.cf.CFRelease(element)

    def has_window(self, app):
        windows, buttons = self.close_buttons(app)
        for button in buttons:
            self.cf.CFRelease(button)
        if windows:
            self.cf.CFRelease(windows)
        return bool(buttons)

    def close(self, app):
        windows, buttons = self.close_buttons(app)
        if not buttons:
            raise RuntimeError("the app's window has no close button")
        press = self.string("AXPress")
        error = self.ax.AXUIElementPerformAction(buttons[0], press)
        for item in [press, *buttons, windows]:
            self.cf.CFRelease(item)
        if error != 0:
            raise RuntimeError(f"pressing the close button failed (AXError {error})")

    def quit(self, app):
        for down in (True, False):
            event = self.ax.CGEventCreateKeyboardEvent(None, self.KEY_Q, down)
            self.ax.CGEventSetFlags(event, self.COMMAND)
            self.ax.CGEventPostToPid(app.pid, event)
            self.cf.CFRelease(event)


def scratch_env(scratch):
    """The app's environment: an E2E build takes its directories from these
    and won't start without them (src-tauri/src/app_dirs.rs)."""
    dirs = {name: os.path.join(scratch, name) for name in ("config", "cache", "documents", "home")}
    for path in dirs.values():
        os.makedirs(path)
    env = dict(os.environ,
               HOPLODEX_E2E_CONFIG_HOME=dirs["config"],
               HOPLODEX_E2E_CACHE_HOME=dirs["cache"],
               HOPLODEX_E2E_DOCUMENTS=dirs["documents"],
               HOPLODEX_E2E_KEYRING_FILE=os.path.join(scratch, "keyring.json"),
               HOPLODEX_E2E_OPENED_LOG=os.path.join(scratch, "opened.log"),
               TAURI_WEBDRIVER_PORT=str(WEBDRIVER_PORT))
    if LINUX:
        env.update(GDK_BACKEND="x11", XDG_DATA_HOME=scratch + "/data",
                   XDG_CACHE_HOME=dirs["cache"], XDG_CONFIG_HOME=dirs["config"])
        env.pop("WAYLAND_DISPLAY", None)
    if MACOS:
        # macOS builds Application Support and the rest from HOME, as in
        # e2e/wdio.conf.ts.
        env.update(HOPLODEX_E2E_HOME=dirs["home"], HOME=dirs["home"], CFFIXED_USER_HOME=dirs["home"])
    return env


def webdriver_answers():
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{WEBDRIVER_PORT}/status", timeout=1) as response:
            return response.status == 200
    except OSError:
        return False


def run(desktop, how):
    scratch = tempfile.mkdtemp(prefix="hoplodex-quit-")
    env = scratch_env(scratch)
    planted = os.path.join(scratch, "cache", IDENTIFIER, "opened-documents", "7", "receipt.pdf")
    app = subprocess.Popen([BINARY], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.time() + 30
        while time.time() < deadline and not (webdriver_answers() and desktop.has_window(app)):
            if app.poll() is not None:
                return False, f"the app exited at launch ({app.returncode})"
            time.sleep(0.5)
        if not desktop.has_window(app):
            return False, "the app's window never appeared"
        time.sleep(3)  # let startup (and its sweep of old copies) finish first

        os.makedirs(os.path.dirname(planted))
        with open(planted, "w") as f:
            f.write("%PDF-1.4 decrypted copy")

        if how == "close":
            desktop.close(app)
        elif how == "quit":
            desktop.quit(app)
        else:
            app.send_signal({"term": signal.SIGTERM, "hup": signal.SIGHUP, "int": signal.SIGINT,
                             "kill": signal.SIGKILL}[how])
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
    except RuntimeError as err:
        return False, str(err)
    finally:
        if app.poll() is None:
            app.kill()
            app.wait()
        shutil.rmtree(scratch, ignore_errors=True)


if __name__ == "__main__":
    if not (LINUX or MACOS):
        sys.exit("Linux and macOS only; on Windows, run quit-cleanup.ps1")
    if not os.path.exists(BINARY):
        sys.exit(f"build the E2E binary first: {BINARY} not found")
    if webdriver_answers():
        sys.exit(f"something already answers on port {WEBDRIVER_PORT}; is another app still running?")
    desktop = X11() if LINUX else MacOS()
    cases = [("close", "window closed")]
    if MACOS:
        cases.append(("quit", "Cmd+Q"))
    cases += [("term", "SIGTERM"), ("hup", "SIGHUP"), ("int", "SIGINT"), ("kill", "SIGKILL (control)")]
    failures = 0
    for how, label in cases:
        ok, detail = run(desktop, how)
        print(f"{'PASS' if ok else 'FAIL'}  {label}: {detail}", flush=True)
        failures += not ok
    sys.exit(1 if failures else 0)
