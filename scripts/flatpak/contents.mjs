// What the Flatpak bundle must carry and must not (011 research.md §13): the
// build's contents check. The check itself is 011 T022's; this is the shape
// `contents.test.ts` (011 T016) calls.

/**
 * @param {{ files: string[], readelf: string, runtimeLibraries: string[] }} inputs
 *   the built app's file paths relative to its `files/`, the output of
 *   `readelf -d bin/hoplodex`, and the library file names the runtime has
 * @returns {string[]} one message per problem; empty when the bundle is right
 */
export function checkContents(inputs) {
  void inputs;
  throw new Error("checkContents is not yet implemented (011 T022)");
}
