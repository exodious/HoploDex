# UI: Troubleshooting Log

## About: "Troubleshooting log" section

Between the program block and the license. It appears whether or not the notices load.

- **Off:** "Off. Turn it on to gather details for a problem you can repeat." Button: **Turn on**.
- **On:** "On." Then the path, selectable. Buttons: **Detailed until you quit**, **Show log file**, **Turn off**.
- **Detailed:** "On, with extra detail until you quit." Then the path. Buttons: **Show log file**, **Turn off**.
- **Failed:** the state line becomes "On, but the file can't be written."
- **Show log file** is disabled when there is no file. If the show fails, a one-line error in the section says why, such as that there is no file.

A short line under the state in every case: "It is designed to leave out your records, file names, passphrases and how many items you have. Read it before you share it."

Focus stays on the section after a press: a button that goes away moves focus to the section's first button. No form, since nothing is typed.

## Chooser notice

The chooser's usual notice banner, with FR-021's text and a small **Turn off** button, like the backup notice's button. After pressing: "The troubleshooting log is off, and its files are deleted."

## Screenshots

`e2e/screenshots/screens.e2e.ts` adds: the notice in the chooser, and About's section off, on and detailed.
