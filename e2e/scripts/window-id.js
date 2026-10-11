// macOS: prints the CGWindowID and bounds (in points) of the main window of
// the process with the given pid, as JSON, for `screencapture -l` (#88):
//
//   osascript -l JavaScript e2e/scripts/window-id.js <pid>
//
// It reads CGWindowListCopyWindowInfo, which lists a window whether or not
// another one covers it, and needs no permission for the owner and bounds
// (only a window's title needs Screen Recording). The main window is the
// biggest one at the normal level (0): the app has no other.

ObjC.import("CoreGraphics");
ObjC.import("Foundation");

function run(argv) {
  const pid = Number(argv[0]);
  const list = ObjC.deepUnwrap(
    ObjC.castRefToObject(
      $.CGWindowListCopyWindowInfo(
        $.kCGWindowListOptionOnScreenOnly | $.kCGWindowListExcludeDesktopElements,
        $.kCGNullWindowID,
      ),
    ),
  );
  let best = null;
  for (const w of list) {
    if (w.kCGWindowOwnerPID !== pid || w.kCGWindowLayer !== 0) continue;
    const b = w.kCGWindowBounds;
    const area = b.Width * b.Height;
    if (!best || area > best.area) {
      best = { id: w.kCGWindowNumber, x: b.X, y: b.Y, width: b.Width, height: b.Height, area };
    }
  }
  if (!best) throw new Error("no window for process " + pid);
  delete best.area;
  return JSON.stringify(best);
}
