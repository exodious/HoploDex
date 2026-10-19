import { describe, expect, it } from "vitest";
import { outOfReachText, overrideCommand } from "./outOfReach";

// 011 FR-004: the flags that carry no message read like the backend's message.
describe("outOfReach wording (011 FR-004, 011 US1-8)", () => {
  it("names the override command for the folder", () => {
    expect(overrideCommand("/srv/hoplodex")).toBe(
      "flatpak override --user --filesystem=/srv/hoplodex io.github.exodious.HoploDex",
    );
  });

  it.each([
    ["database", "This copy of HoploDex can't reach this database's folder.", "Move the file into"],
    ["folder", "This copy of HoploDex can't reach this folder.", "Choose a folder in"],
  ] as const)("says what can't be reached, and both fixes, for a %s", (subject, cant, move) => {
    const text = outOfReachText(subject, "/srv/hoplodex");

    expect(text).toContain(cant);
    expect(text).toContain(
      "It is a Flatpak, and it can only reach your home folder and removable drives.",
    );
    expect(text).toContain(`${move} your home folder`);
    expect(text).toContain(
      "give HoploDex access to /srv/hoplodex by running: flatpak override --user --filesystem=/srv/hoplodex io.github.exodious.HoploDex and then restarting HoploDex.",
    );
  });
});
