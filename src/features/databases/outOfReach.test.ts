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
    ["a folder with a space", "/media/sam/My Drive", "'/media/sam/My Drive'"],
    ["a single quote", "/srv/Sam's guns", "'/srv/Sam'\\''s guns'"],
    ["a dollar sign and a double quote", '/srv/$HOME/"x"', `'/srv/$HOME/"x"'`],
    ["a backslash and a semicolon", "/srv/a\\b;rm", "'/srv/a\\b;rm'"],
  ])("quotes %s so a shell reads the folder as one word", (_name, folder, quoted) => {
    expect(overrideCommand(folder)).toBe(
      `flatpak override --user --filesystem=${quoted} io.github.exodious.HoploDex`,
    );
  });

  it.each([
    ["database", "This copy of HoploDex can't reach this database's folder.", "Move the file into"],
    ["folder", "This copy of HoploDex can't reach this folder.", "Choose a folder in"],
  ] as const)("says what can't be reached, and both fixes, for a %s", (subject, cant, move) => {
    const text = outOfReachText(subject, "/srv/hoplodex");

    expect(text).toContain(cant);
    expect(text).toContain(
      "It is a Flatpak, and by default it can reach only the HoploDex folder in your Documents folder and removable drives.",
    );
    // 011 T099: the grants can be changed, so the flag says only what is reachable by default.
    expect(text).not.toContain("can only reach");
    expect(text).toContain(`${move} the HoploDex folder in your Documents folder`);
    expect(text).toContain(
      "give HoploDex access to /srv/hoplodex by running: flatpak override --user --filesystem=/srv/hoplodex io.github.exodious.HoploDex and then restarting HoploDex.",
    );
  });
});
