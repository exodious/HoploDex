# Data Model: Troubleshooting Log

## `machine.json` (no change to `VERSION`: both read as absent from older files)

| Field | Type | Meaning |
|---|---|---|
| `troubleshootingLog` | `"on"`, `"off"` or absent | The user's saved choice. Absent means follow the version's default. A value this version doesn't know reads as absent. |
| `troubleshootingLogNoticeShown` | boolean, default false | The notice has been queued on this computer. |

## `ChooserNotice::TroubleshootingLog`

A new kind, `{"kind":"troubleshootingLog"}`, with no fields. An older version reading it drops it (009 research.md §15).

## `LogState`

`off`, `on`, `detailed`. What applies now. Never saved as such: only `on` and `off` become a choice, and only through FR-003's control.

## How the state is decided at start

| Variable | Saved | Version | State | Leftover files | Notice |
|---|---|---|---|---|---|
| none | none | 0.x | on | kept, aged | once |
| none | none | ≥1 | off | deleted | no |
| none | on | any | on | kept, aged | no |
| none | off | any | off | deleted | no |
| `off` | any | any | off | left alone | no |
| `on` | any | any | on | kept, aged | no |
| `debug` | any | any | detailed | kept, aged | no |
| unknown | | | as if none, with a line | | |

## Log file

Plain text, one line per entry:

```text
--- HoploDex 0.1.0 on linux x86_64, started 2026-11-07 ---
+0.012 [INFO] the helpers are confined with Landlock ABI 6
+4.310 [DEBUG] opening ~/*/*.hoplodex (take over: false)
```

Shown on the page as a path only. The page never receives contents. Paths in this table of lines are shapes (`logging::place`); on Windows the separators are backslashes.
