# Contract: About HoploDex

**Feature**: [spec.md](../spec.md) | **Research**: [research.md](../research.md) §10, §11, §13, §15

## §1 Ways in (FR-001)

| Where | Control | Label | Shown |
|---|---|---|---|
| Database chooser, top bar, before the theme toggle | `Button` `variant="ghost"`, as the top bar's other tools | "About HoploDex" | Always, in the chooser phase |
| Database menu (`DatabaseMenu`) | `MenuItem`, after "About databases and security" | "About HoploDex" | While a database is open |
| macOS application menu | `MenuItem` id `about`, in place of the predefined About | "About HoploDex" | Always, on macOS only |

Each calls `session.showAbout()`, which opens the one `AboutDialog` rendered
by `SessionProvider`. When About is already open, a second call leaves it as
it is. If another dialog is open when the macOS item is chosen, About opens
over it, and closing About returns focus there.

About closes, without asking, on any change of session phase: an open, a
lock, a close, a database switch, or the closing screen (research.md §11).

## §2 Layout

The shared `Dialog`, `size="lg"`, title "About HoploDex", no footer. Its body
scrolls (`.hd-dialog__body`), and the header with its Close button stays put.
From top to bottom:

1. **Program block** (always shown, from compiled-in constants):
   - "HoploDex" as an `h3`, with "Version 0.1.0" beside it.
   - "Copyright © 2026 exodious and the HoploDex contributors". The years are
     "2026–YYYY" from the year after 2026 on.
   - "HoploDex is free software: you can share it and change it under the
     terms of the GNU General Public License, version 3 only, as published by
     the Free Software Foundation."
   - "It comes with no warranty, to the extent permitted by law. See the
     license for details."
   - The additional permission (`notices.json`'s `additionalPermission`), in
     the body's smaller text style, once the notices have loaded. Its heading
     line is shown as a `strong` run-in.
   - "Source code:", then `https://github.com/exodious/HoploDex` in a
     monospace span with `user-select: all`, as export's paths are shown
     (`.hd-paths dd`). Not a link, and nothing opens it (FR-006).
2. **The license**: a `Disclosure` (`headingLevel={3}`), titled "GNU General
   Public License, version 3", with the summary "The full text of the license
   HoploDex is under." Opened, it shows `notices.json`'s `gpl` in a `pre`
   block (`.hd-about__text`): monospace, wrapping, `max-height: 60vh`, its own
   scroll, `tabIndex={0}` with an accessible name, so the keyboard can scroll
   it.
3. **"Built with"** (`h3`), with the sentence "HoploDex includes these
   components by other authors. Each is used under the license shown, whose
   text follows its name."
   - **The filter**: a `TextField`, label "Find a component", `type="search"`.
     Escape in it clears it when it has text, and otherwise closes the dialog
     (the dialog's own Escape).
   - **The count**, a polite live region: "324 components", or "3 of 324
     components" while filtering.
   - **The list**: one `Disclosure` (`headingLevel={4}`) per component, in
     `notices.json`'s order. Title: the name. Summary: "version · license",
     plus ", used under MIT" when the package offers a choice. Opened, it
     shows each of its texts in order, in `.hd-about__text` blocks.
     Hand-recorded components add one line saying what carries them: "Built
     into libsqlite3-sys 0.38.1".
   - **No match**: "No component matches “<filter>”." in place of the list.
4. **While loading**: sections 2 and 3 show one line, "Loading the license
   notices…", after 1 s. That is the closing screen's show-after-a-moment rule
   (`SHOW_AFTER_MS`), so the usual load, in milliseconds, doesn't flash.
5. **If loading fails**: sections 2 and 3 show, in the error banner style,
   "The license notices couldn't be loaded. They are also in the NOTICES.txt
   file that came with HoploDex." The program block stays.

At the narrow width (the app's `max-width: 900px` breakpoint, and the
dialog's own small-window rules), everything is one column,
the dialog takes the window's width less the gutters, and the text blocks wrap
(FR-007).

## §3 Keyboard and focus (FR-007; One Enter rule)

- **On open**: focus goes to the first control in the body, the license
  disclosure's button. That is the shared `Dialog`'s rule (`FIRST_FIELD`), as
  for "About databases and security". The program block above it is text the
  reader reaches by reading. The filter follows, by Tab.
- **Enter and Space** on a disclosure's button toggle it. Enter in the filter
  does nothing, because About has no primary action, so it isn't a `<form>`.
- **Escape** closes the dialog, unless the filter has text, which Escape
  clears first. Focus returns to the control that opened it: the chooser's
  button, or the database menu's trigger, as `Dialog` restores it.
- **The `/` shortcut** stays inert while About is open, as for any dialog
  (`useSearchShortcut`).
- **Contrast and roles**: the existing tokens, at WCAG 2.1 AA in both themes.
  Each text block is `role="region"` with the name "<component> license text"
  (or "GNU General Public License text").

## §4 The macOS menu event

`app:about-requested`, emitted by the backend with no payload when the
application menu's `about` item is chosen (contracts/tauri-commands.md).
`SessionProvider` subscribes for its whole life, and also during the starting
phase, through `sessionService.onAboutRequested`, beside `onQuitRequested`.

## §5 Screenshots (the walk)

`e2e/screenshots/screens.e2e.ts`. Numbers are the walk's next free ones at
implementation:

| Shot | State |
|---|---|
| `NN-about-chooser-{light,dark}` | Chooser, About open, at the top |
| `NN-about-license-{light,dark}` | The license disclosure opened |
| `NN-about-component-{light,dark}` | Filter "sqlcipher", its row opened |
| `NN-about-no-match-{light,dark}` | Filter "zzzz" |
| `NN-about-narrow-{light,dark}` | At the narrow width, from the database menu |

## §6 What the strength hint changes

Nothing visible. The meter and its suggestions keep their wording.
`wikipedia.json`'s words now rate as their other matches allow.
