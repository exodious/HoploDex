#!/usr/bin/env python3
"""Real mouse and keyboard input for scripts/pdf-surface-check.sh and
scripts/window-controls-check.sh on Linux, the counterpart of real_input.swift
(macOS) and real_input.ps1 (Windows). It wraps e2e/scripts/x11-input.py (XTest,
as the E2E suite's real input does) and adds the right button, the wheel and
the mouse's back and forward buttons, which that one lacks.

    real_input.py click X Y       left click at screen coordinates
    real_input.py rclick X Y      right click
    real_input.py move X Y        move the pointer
    real_input.py scroll X Y N    N wheel notches (negative: up)
    real_input.py key Control_L+s X keysyms, + for a chord
    real_input.py xbutton X Y back|forward
                                         the mouse's side buttons (8 and 9)
    real_input.py drag X1 Y1 X2 Y2 [VX VY]
                                         press the left button at X1 Y1, move to
                                         X2 Y2 in steps (by way of VX VY) and
                                         release there
"""

import importlib.util
import pathlib
import sys
import time

# Leave no __pycache__ in e2e/scripts/ (the checkout is not ours to litter).
sys.dont_write_bytecode = True

HERE = pathlib.Path(__file__).resolve()
spec = importlib.util.spec_from_file_location(
    "x11_input", HERE.parents[2] / "e2e" / "scripts" / "x11-input.py"
)
x11_input = importlib.util.module_from_spec(spec)
spec.loader.exec_module(x11_input)
x11, xtst = x11_input.x11, x11_input.xtst


def with_display(action):
    display = x11.XOpenDisplay(None)
    if not display:
        sys.exit("real_input: can't open the display in $DISPLAY")
    action(display)
    x11.XSync(display, 0)
    x11.XCloseDisplay(display)


def move(display, x, y):
    xtst.XTestFakeMotionEvent(display, -1, x, y, 0)
    x11.XFlush(display)
    time.sleep(0.08)


def button(display, number, count=1):
    for _ in range(count):
        xtst.XTestFakeButtonEvent(display, number, True, 0)
        x11.XFlush(display)
        time.sleep(0.05)
        xtst.XTestFakeButtonEvent(display, number, False, 0)
        x11.XFlush(display)
        time.sleep(0.04)


def main(args):
    command = args[0] if args else ""
    if command in ("move", "click", "rclick") and len(args) == 3:
        x, y = int(args[1]), int(args[2])

        def act(display):
            move(display, x, y)
            if command != "move":
                button(display, 1 if command == "click" else 3)

        with_display(act)
    elif command == "scroll" and len(args) == 4:
        x, y, notches = int(args[1]), int(args[2]), int(args[3])
        with_display(lambda d: (move(d, x, y), button(d, 4 if notches < 0 else 5, abs(notches))))
    elif command == "xbutton" and len(args) == 4 and args[3] in ("back", "forward"):
        x, y, number = int(args[1]), int(args[2]), 8 if args[3] == "back" else 9

        def act(display):
            move(display, x, y)
            button(display, number)

        with_display(act)
    elif command == "drag" and len(args) in (5, 7):
        points = [int(v) for v in args[1:]]
        x1, y1, x2, y2 = points[:4]
        legs = [(x1, y1, *points[4:6]), (*points[4:6], x2, y2)] if len(points) == 6 else [
            (x1, y1, x2, y2)
        ]

        def act(display):
            move(display, x1, y1)
            xtst.XTestFakeButtonEvent(display, 1, True, 0)
            x11.XFlush(display)
            time.sleep(0.2)
            steps = 30
            for ax, ay, bx, by in legs:
                for i in range(1, steps + 1):
                    xtst.XTestFakeMotionEvent(
                        display, -1, ax + (bx - ax) * i // steps, ay + (by - ay) * i // steps, 0
                    )
                    x11.XFlush(display)
                    time.sleep(0.03)
            # Rest over the target, so it sees the drag before the release.
            time.sleep(0.5)
            xtst.XTestFakeMotionEvent(display, -1, x2 + 1, y2 + 1, 0)
            x11.XFlush(display)
            time.sleep(0.3)
            xtst.XTestFakeButtonEvent(display, 1, False, 0)
            x11.XFlush(display)
            time.sleep(0.3)

        with_display(act)
    elif command == "key" and len(args) == 2:
        x11_input.main(["key", args[1]])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
