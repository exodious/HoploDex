# Commands: Troubleshooting Log

All three are unscoped: no database need be open, and none touches one.

## `get_troubleshooting_log() -> TroubleshootingLog`

```ts
interface TroubleshootingLog {
  state: "off" | "on" | "detailed";
  /** The file's real path. Null when the app's folders can't be named. */
  path: string | null;
  /** Whether a log file is there now. */
  exists: boolean;
  /** The log is meant to be on, but the file can't be written. */
  failed: boolean;
}
```

## `set_troubleshooting_log(state) -> TroubleshootingLog`

`state` is `"off"`, `"on"` or `"detailed"`, and any other value is refused by deserialization.

| Now | Asked | Effect |
|---|---|---|
| off | `on` | Starts the file. Saves On. |
| off | `detailed` | Nothing: refused by Rust, whatever the page shows. Returns the current state. |
| on | `detailed` | Raises the level. Saves nothing. |
| detailed | `on` | Nothing: a raise lasts until quit. Returns the current state. |
| on or detailed | `off` | Stops the file, deletes both files. Saves Off. |
| same as now | same | Nothing. |

## `show_troubleshooting_log() -> ()`

Takes no argument. Rust picks the path. Shows the file in the OS's file manager, through `Opener::reveal`. Errors: `NOT_FOUND` when no plain file of this user's is there (a link planted at the name counts as none); `INTERNAL_ERROR` when the OS can't show it, with a message that names no path.
