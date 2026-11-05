import { execFileSync } from "node:child_process";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { startDisplay, stopDisplay } from "./display";

/** Connects to `DISPLAY` as a client of its own and interns an atom of this
 * test's, creating it if `create` is set, else only looking it up. Returns
 * the atom's number, "0" when it doesn't exist. */
function internAtom(create: boolean): string {
  const script = `
import ctypes, sys
x11 = ctypes.CDLL("libX11.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
x11.XInternAtom.restype = ctypes.c_ulong
display = x11.XOpenDisplay(None)
if not display:
    sys.exit("no X display")
print(x11.XInternAtom(display, b"HOPLODEX_DISPLAY_TEST", ${create ? 0 : 1}))
`;
  return execFileSync("python3", ["-c", script], { encoding: "utf-8" }).trim();
}

const saved = { ...process.env };

describe.runIf(process.platform === "linux")("the E2E worker's display", () => {
  beforeAll(startDisplay);
  afterAll(async () => {
    await stopDisplay();
    process.env = { ...saved };
  });

  // An X server resets when its last client leaves, unless started with
  // -noreset, and a reset frees every atom a client interned (the X protocol,
  // "Connection Close"). The reset also drops a connection still being set
  // up, which failed the app's GTK init when its sandbox probe left the
  // display just before it.
  it("doesn't reset when its last client leaves", () => {
    expect(internAtom(true)).not.toBe("0");
    expect(internAtom(false)).not.toBe("0");
  });
});
