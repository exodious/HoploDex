#!/usr/bin/env python3
"""Sends real keyboard and mouse input to the app on the X display in
$DISPLAY, through the XTest extension.

WebDriver's own clicks and keys don't pass through GTK the way a person's do,
so WebKitGTK behaviour that depends on real input (whether a focus shows its
ring, `:focus-visible`, a relayout a key press forces) doesn't reproduce with
them. This does. `e2e/support/realInput.ts` drives it from a spec; see
"Real keyboard and mouse input" in DEVELOPMENT.md.

Needs libX11 and libXtst (both in the dev container) and python3 only.

    x11-input.py move X Y        # pointer to screen coordinates
    x11-input.py click           # left button, pressed and released
    x11-input.py key Tab         # X keysym names; + for a chord
    x11-input.py key Shift_L+Tab
"""

import ctypes
import sys
import time
from ctypes import c_int, c_uint, c_ulong, c_void_p

x11 = ctypes.CDLL("libX11.so.6")
xtst = ctypes.CDLL("libXtst.so.6")
x11.XOpenDisplay.restype = c_void_p
x11.XFlush.argtypes = [c_void_p]
x11.XSync.argtypes = [c_void_p, c_int]
x11.XCloseDisplay.argtypes = [c_void_p]
x11.XStringToKeysym.restype = c_ulong
x11.XKeysymToKeycode.argtypes = [c_void_p, c_ulong]
xtst.XTestFakeMotionEvent.argtypes = [c_void_p, c_int, c_int, c_int, c_ulong]
xtst.XTestFakeButtonEvent.argtypes = [c_void_p, c_uint, c_int, c_ulong]
xtst.XTestFakeKeyEvent.argtypes = [c_void_p, c_uint, c_int, c_ulong]


def main(args: list[str]) -> None:
    display = x11.XOpenDisplay(None)
    if not display:
        sys.exit("x11-input: can't open the display in $DISPLAY")
    command = args[0] if args else ""
    if command == "move" and len(args) == 3:
        xtst.XTestFakeMotionEvent(display, -1, int(args[1]), int(args[2]), 0)
    elif command == "click":
        xtst.XTestFakeButtonEvent(display, 1, True, 0)
        x11.XFlush(display)
        time.sleep(0.05)
        xtst.XTestFakeButtonEvent(display, 1, False, 0)
    elif command == "key" and len(args) == 2:
        codes = [x11.XKeysymToKeycode(display, x11.XStringToKeysym(name.encode())) for name in args[1].split("+")]
        if 0 in codes:
            sys.exit(f"x11-input: unknown key in {args[1]!r}")
        for code in codes:
            xtst.XTestFakeKeyEvent(display, code, True, 0)
            x11.XFlush(display)
            time.sleep(0.02)
        for code in reversed(codes):
            xtst.XTestFakeKeyEvent(display, code, False, 0)
            x11.XFlush(display)
            time.sleep(0.02)
    else:
        sys.exit(__doc__)
    # Wait until the server has processed every event before exiting: with
    # only a flush, the last one (a chord's Shift release) was sometimes lost,
    # leaving Shift held for every later key.
    x11.XSync(display, 0)
    x11.XCloseDisplay(display)


if __name__ == "__main__":
    main(sys.argv[1:])
