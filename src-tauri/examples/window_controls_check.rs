// The window controls check (specs/008-hardening-batch/research.md §13, §14,
// §18, quickstart.md "The window controls check", tasks T081 and T083 to T085):
// the app's own `window_controls::apply` on a main window opened the way the
// E2E suite isolates one (scratch config, cache and data folders), showing a
// test page, and driven with real input. It checks SC-005 on each OS.
//
//   cargo run --profile e2e --example window_controls_check --features e2e -- \
//       --input CMD [--input ARG]... [--scratch DIR] [--out DIR] [--timeout SECONDS]
//
// It must be built without debug assertions (`--profile e2e` or `--release`):
// a development build installs none of the filters (FR-014), and the check
// says so and exits 3. `--input` is the command that posts real input, given
// `click X Y`, `rclick X Y`, `move X Y`, `xbutton X Y back|forward` and `key
// ...` after it, with screen coordinates: examples/real_input.py (Linux),
// real_input.swift (macOS, compiled) or real_input.ps1 (Windows). On macOS it
// also takes `windows PID`.
//
// The test page has a blank area, a table-row-like block, a button, an image,
// a text field holding text and a paragraph, a marker in `sessionStorage` and
// a counter of loads. It has none of the app's own page handlers
// (`src/features/app/browserControls.ts`), so what it checks is the native
// layer alone, which is the stricter test: a page that cancels `contextmenu`
// can hide a menu the native layer would have shown.
//
// The check:
//   1. right-clicks each target and logs every item the native menu offered,
//      from the filter's own log (`window_controls::apply(.., log_items:
//      true)`: "context menu item ..." on Linux, "menu item ..." on macOS,
//      and on Windows lines naming `name=<item name>`; each ends in "kept" or
//      "removed");
//   2. types in the text field, then presses every FR-012 key and the mouse's
//      back and forward buttons, and after each types a letter and deletes it:
//      the letter must reach the field (nothing took the keyboard);
//   3. on macOS, selects the paragraph and presses control-command-D, and
//      counts the app's windows before and after (a Look Up panel is one).
// It fails (exit 4, one "CHECK FAIL" line each) if:
//   - a menu kept an item outside FR-011's list (macOS's AutoFill, which
//     AppKit adds outside the menu's items, is not in the log; spec.md
//     Clarifications 2026-10-11);
//   - a menu was offered where none belongs (a blank area, a row, a button
//     or an image), or none on the text field or the selection;
//   - the page loaded again (a reload or navigation: the load counter in
//     `sessionStorage`), a window was asked for, a key took the keyboard from
//     the field, or the typed text was lost;
//   - a Look Up panel opened (macOS).
//
// Exit codes:
//   0  passed
//   1  the window couldn't be built or its page didn't load
//   3  usage (no --input, or a build without the filters)
//   4  a check failed
// Everything is logged to stderr with a "CHECK" prefix. Nothing outside the
// scratch folder is touched. On Linux run it with
// `scripts/dev-container.sh scripts/window-controls-check.sh`.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use hoplodex_lib::window_controls;
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Wry, http, webview::NewWindowResponse,
};

/// The main window's page comes from a protocol (`data:` URLs need a Tauri
/// feature the app doesn't use).
const MAIN_SCHEME: &str = "hdmain";

const PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>window controls check</title>
<style>
  body { margin: 0; font: 16px sans-serif; background: #eef; }
  .t { position: absolute; box-sizing: border-box; }
  #row { left: 20px; top: 20px; width: 400px; height: 40px; background: #fff;
    border: 1px solid #888; line-height: 38px; padding-left: 8px;
    user-select: none; -webkit-user-select: none; }
  #btn { left: 20px; top: 80px; width: 300px; height: 40px;
    user-select: none; -webkit-user-select: none; }
  #img { left: 20px; top: 140px; width: 100px; height: 100px; user-select: none;
    -webkit-user-select: none; }
  #field { left: 20px; top: 260px; width: 400px; height: 40px; font-size: 16px; }
  #para { left: 20px; top: 330px; width: 400px; height: 40px; margin: 0;
    line-height: 40px; }
</style>
<div id="row" class="t">Glock 19 Gen5 (a table row)</div>
<button id="btn" class="t">A button</button>
<img id="img" class="t" alt="" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='100' height='100'%3E%3Crect width='100' height='100' fill='%23c33'/%3E%3C/svg%3E">
<input id="field" class="t" value="the quick brwon fox" spellcheck="true">
<p id="para" class="t">Select this sentence and right-click it</p>
<script>
  const id = Math.random().toString(36).slice(2);
  // A reload or a navigation loads the page again: the counter survives it
  // (same tab), the id and the field's text don't.
  const loads = Number(sessionStorage.getItem('loads') || 0) + 1;
  sessionStorage.setItem('loads', String(loads));
  fetch('/loaded?id=' + id + '&n=' + loads);
  setInterval(() => fetch('/tick?id=' + id + '&v=' + encodeURIComponent(field.value)), 400);
  function selectPara() { field.blur(); getSelection().selectAllChildren(para); }
</script>"#;

/// Where the targets are, in logical pixels of the window's content area: the
/// middle of each.
const BLANK: (f64, f64) = (800.0, 600.0);
const ROW: (f64, f64) = (200.0, 40.0);
const BUTTON: (f64, f64) = (170.0, 100.0);
const IMAGE: (f64, f64) = (70.0, 190.0);
const FIELD: (f64, f64) = (210.0, 280.0);
/// Right of the field's text, so a click there puts the caret at its end.
const FIELD_END: (f64, f64) = (400.0, 280.0);
const PARA: (f64, f64) = (200.0, 350.0);

// ---------------------------------------------------------------------------
// What the native filters log
// ---------------------------------------------------------------------------

/// The lines `window_controls` logged (`log::info!`), in order.
static NATIVE: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let from = record.module_path().unwrap_or("");
        if from.contains("window_controls") || record.target().contains("window_controls") {
            let line = record.args().to_string();
            eprintln!("CHECK native: {line}");
            NATIVE.lock().unwrap().push(line);
        }
    }

    fn flush(&self) {}
}

/// One context-menu item the filter saw.
#[derive(Debug)]
struct Item {
    text: String,
    kept: bool,
}

/// The items among the lines logged since `from`. A line is an item when it
/// ends in "(kept)" or "(removed)" (Linux) or "-> kept" or "-> removed"
/// (macOS, and Windows lines in either form); separators and the lines for
/// what is inside an item that is going are not.
fn items_since(from: usize) -> Vec<Item> {
    NATIVE
        .lock()
        .unwrap()
        .iter()
        .skip(from)
        .filter(|line| !line.starts_with("  inside") && !line.contains("separator=true"))
        .filter(|line| !line.starts_with("context menu item: separator"))
        .filter_map(|line| {
            let kept = line.ends_with("(kept)") || line.ends_with("-> kept");
            let removed = line.ends_with("(removed)") || line.ends_with("-> removed");
            (kept || removed).then(|| Item { text: line.clone(), kept })
        })
        .collect()
}

/// FR-011's list, independent of the filters' own: whether a kept item is
/// one of cut, copy, paste, delete, select all, undo, redo, the spelling
/// items and emoji. `line` is the filter's log line.
fn in_fr011_list(line: &str) -> bool {
    if cfg!(target_os = "linux") {
        // "context menu item: Cut \"\" (kept)": the stock action, then the
        // name of a `Custom` item's action.
        let rest = line.trim_start_matches("context menu item: ");
        let stock = rest.split_whitespace().next().unwrap_or("");
        match stock {
            "Cut" | "Copy" | "Paste" | "Delete" | "SelectAll" | "InsertEmoji" | "SpellingGuess"
            | "NoGuessesFound" | "IgnoreSpelling" | "LearnSpelling" | "IgnoreGrammar" | "Undo"
            | "Redo" => true,
            "Custom" => {
                let lower = line.to_ascii_lowercase();
                lower.contains("spell") || lower.contains("grammar")
            }
            _ => false,
        }
    } else if cfg!(target_os = "macos") {
        const IDENTIFIERS: &[&str] = &[
            "WKMenuItemIdentifierCut",
            "WKMenuItemIdentifierCopy",
            "WKMenuItemIdentifierPaste",
            "WKMenuItemIdentifierSpellingMenu",
            "WKMenuItemIdentifierSpellingGuess",
            "WKMenuItemIdentifierIgnoreSpelling",
            "WKMenuItemIdentifierLearnSpelling",
            "WKMenuItemIdentifierShowSpellingPanel",
            "WKMenuItemIdentifierCheckSpellingWhileTyping",
        ];
        if IDENTIFIERS.iter().any(|id| line.contains(&format!("\"{id}\""))) {
            return true;
        }
        if line.contains("orderFrontCharacterPalette:") {
            return true;
        }
        // Items WebKit gives no identifier: Cut, the guesses, Ignore and Learn
        // Spelling, found by tag (research.md §13).
        line.contains("forwardContextMenuAction:")
            && [13, 15, 17, 18].iter().any(|tag| line.contains(&format!("tag={tag} ")))
    } else {
        // Windows names each item (`name=<name>` in the line, T079).
        const NAMES: &[&str] =
            &["cut", "copy", "paste", "selectAll", "undo", "redo", "emoji", "spellCheck"];
        line.split_whitespace()
            .filter_map(|word| word.strip_prefix("name="))
            .any(|name| NAMES.contains(&name.trim_matches('"')))
    }
}

// ---------------------------------------------------------------------------
// What the page tells us
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Shared {
    /// `(page load id, the load counter in sessionStorage)` per load.
    loads: Mutex<Vec<(String, String)>>,
    /// The latest tick: `(load id, the field's value)`.
    tick: Mutex<Option<(String, String)>>,
    ticks: AtomicUsize,
    new_windows: AtomicUsize,
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(query: &str, name: &str) -> String {
    query
        .split('&')
        .find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == name).then(|| percent_decode(value))
        })
        .unwrap_or_default()
}

fn serve(shared: &Shared, req: &http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let query = req.uri().query().unwrap_or("");
    let empty = || http::Response::builder().body(Vec::new()).unwrap();
    match req.uri().path() {
        "/loaded" => {
            let load = (query_param(query, "id"), query_param(query, "n"));
            eprintln!("CHECK page loaded: id {} (load {})", load.0, load.1);
            shared.loads.lock().unwrap().push(load);
            empty()
        }
        "/tick" => {
            shared.ticks.fetch_add(1, Ordering::SeqCst);
            *shared.tick.lock().unwrap() =
                Some((query_param(query, "id"), query_param(query, "v")));
            empty()
        }
        _ => http::Response::builder()
            .header("Content-Type", "text/html")
            .body(PAGE.as_bytes().to_vec())
            .unwrap(),
    }
}

// ---------------------------------------------------------------------------
// Real input
// ---------------------------------------------------------------------------

/// Posts real mouse and keyboard input through the command `--input` names.
struct Input {
    command: Vec<String>,
    /// Where the window's content area starts on the screen, in the units the
    /// input command takes, and how many of those a logical pixel is.
    origin: (f64, f64),
    unit: f64,
}

impl Input {
    fn run(&self, args: &[String]) -> Option<String> {
        let (program, rest) = self.command.split_first().expect("a command");
        match Command::new(program).args(rest).args(args).output() {
            Ok(out) if out.status.success() => {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            }
            other => {
                eprintln!("CHECK input {args:?} failed: {other:?}");
                None
            }
        }
    }

    fn words(&self, line: &str) {
        self.run(&line.split_whitespace().map(String::from).collect::<Vec<_>>());
    }

    fn at(&self, (x, y): (f64, f64)) -> [String; 2] {
        [
            format!("{}", (self.origin.0 + x * self.unit).round() as i64),
            format!("{}", (self.origin.1 + y * self.unit).round() as i64),
        ]
    }

    fn mouse(&self, command: &str, at: (f64, f64), extra: &[&str]) {
        let [x, y] = self.at(at);
        let mut args = vec![command.to_string(), x, y];
        args.extend(extra.iter().map(|s| s.to_string()));
        self.run(&args);
    }
}

/// One key, as each OS's input command takes it: the Linux X keysyms, the
/// macOS virtual key code with its modifiers, the Windows SendKeys string or
/// `vkey CODE`. An empty one doesn't apply on that OS.
struct KeyCase {
    name: &'static str,
    linux: &'static str,
    mac: &'static str,
    windows: &'static str,
}

const fn key(
    name: &'static str,
    linux: &'static str,
    mac: &'static str,
    windows: &'static str,
) -> KeyCase {
    KeyCase { name, linux, mac, windows }
}

/// FR-012: the keys that reload or move through history, and the web view's
/// shortcuts for printing, saving, source, find and developer tools.
const KEYS: &[KeyCase] = &[
    key("F5", "key F5", "key 96", "key {F5}"),
    key("Shift+F5", "key Shift_L+F5", "key 96 shift", "key +{F5}"),
    key("Ctrl+F5", "key Control_L+F5", "key 96 ctrl", "key ^{F5}"),
    key("Ctrl/Cmd+R", "key Control_L+r", "key 15 cmd", "key ^r"),
    key("Ctrl/Cmd+Shift+R", "key Control_L+Shift_L+r", "key 15 cmd+shift", "key ^+r"),
    key("Alt+Left", "key Alt_L+Left", "", "key %{LEFT}"),
    key("Alt+Right", "key Alt_L+Right", "", "key %{RIGHT}"),
    key("Cmd+[", "", "key 33 cmd", ""),
    key("Cmd+]", "", "key 30 cmd", ""),
    key("Browser Back", "key XF86Back", "", "vkey A6"),
    key("Browser Forward", "key XF86Forward", "", "vkey A7"),
    key("Browser Refresh", "key XF86Reload", "", "vkey A8"),
    key("Ctrl/Cmd+P", "key Control_L+p", "key 35 cmd", "key ^p"),
    key("Ctrl/Cmd+S", "key Control_L+s", "key 1 cmd", "key ^s"),
    key("Ctrl/Cmd+U", "key Control_L+u", "key 32 cmd", "key ^u"),
    key("Ctrl/Cmd+F", "key Control_L+f", "key 3 cmd", "key ^f"),
    key("F3", "key F3", "key 99", "key {F3}"),
    key("Ctrl/Cmd+G", "key Control_L+g", "key 5 cmd", "key ^g"),
    key("F7", "key F7", "key 98", "key {F7}"),
    key("F12", "key F12", "key 111", "key {F12}"),
    key("Ctrl/Cmd+Shift+I", "key Control_L+Shift_L+i", "key 34 cmd+shift", "key ^+i"),
    key("Ctrl/Cmd+Shift+J", "key Control_L+Shift_L+j", "key 38 cmd+shift", "key ^+j"),
    key("Ctrl/Cmd+Shift+C", "key Control_L+Shift_L+c", "key 8 cmd+shift", "key ^+c"),
    key("Cmd+Option+I", "", "key 34 cmd+alt", ""),
];

impl KeyCase {
    fn input(&self) -> &'static str {
        if cfg!(target_os = "linux") {
            self.linux
        } else if cfg!(target_os = "macos") {
            self.mac
        } else {
            self.windows
        }
    }
}

/// A letter to type, to delete again, and Escape, in each OS's form.
fn letter() -> &'static str {
    if cfg!(target_os = "linux") {
        "key z"
    } else if cfg!(target_os = "macos") {
        "key 6"
    } else {
        "key z"
    }
}

fn backspace() -> &'static str {
    if cfg!(target_os = "linux") {
        "key BackSpace"
    } else if cfg!(target_os = "macos") {
        "key 51"
    } else {
        "key {BS}"
    }
}

fn escape() -> &'static str {
    if cfg!(target_os = "linux") {
        "key Escape"
    } else if cfg!(target_os = "macos") {
        "key 53"
    } else {
        "key {ESC}"
    }
}

// ---------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------

struct Options {
    out: PathBuf,
    timeout: Duration,
    input: Vec<String>,
}

fn usage() -> ! {
    eprintln!(
        "usage: window_controls_check --input CMD [--input ARG]... [--scratch DIR] [--out DIR] \
         [--timeout SECONDS]"
    );
    std::process::exit(3)
}

/// Saves the whole screen with the OS's own tool. Best effort.
fn screenshot(path: &Path) -> bool {
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).ok();
    let status = if cfg!(target_os = "linux") {
        Command::new("import").args(["-window", "root"]).arg(path).status()
    } else if cfg!(target_os = "macos") {
        Command::new("screencapture").arg("-x").arg(path).status()
    } else {
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
             $b=[System.Windows.Forms.SystemInformation]::VirtualScreen; \
             $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height; \
             [System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size); \
             $bmp.Save('{}')",
            path.display()
        );
        Command::new("powershell").args(["-NoProfile", "-Command", &script]).status()
    };
    status.map(|s| s.success()).unwrap_or(false) && path.exists()
}

fn wait_until(limit: Duration, condition: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    condition()
}

fn exit_with(app: &AppHandle<Wry>, code: i32) {
    // `app.exit` ends the process with 0 on some OSes whatever the code.
    if code != 0 {
        std::process::exit(code);
    }
    app.exit(0);
}

struct Check {
    app: AppHandle<Wry>,
    shared: Arc<Shared>,
    options: Options,
    input: Input,
    failures: Vec<String>,
    /// How many page loads and new-window requests have been accounted for.
    loads_seen: usize,
    windows_seen: usize,
}

impl Check {
    fn fail(&mut self, what: String) {
        eprintln!("CHECK FAIL: {what}");
        if self.failures.is_empty() {
            // The screen at the first failure.
            let shot = self.options.out.join(format!("failed-{}.png", std::env::consts::OS));
            screenshot(&shot);
        }
        self.failures.push(what);
    }

    fn eval(&self, script: &str) {
        if let Some(main) = self.app.get_webview("main") {
            let _ = main.eval(script);
        }
    }

    /// The field's text as the page last reported it.
    fn field(&self) -> String {
        self.shared.tick.lock().unwrap().clone().map(|(_, value)| value).unwrap_or_default()
    }

    fn field_is(&self, value: &str, limit: Duration) -> bool {
        wait_until(limit, || self.field() == value)
    }

    /// After something that must not reload, navigate or open a window.
    fn assert_stable(&mut self, what: &str) {
        let loads = self.shared.loads.lock().unwrap().len();
        if loads != self.loads_seen {
            self.fail(format!("{what} loaded the page again ({loads} loads)"));
            self.loads_seen = loads;
        }
        let windows = self.shared.new_windows.load(Ordering::SeqCst);
        if windows != self.windows_seen {
            self.fail(format!("{what} asked for a new window"));
            self.windows_seen = windows;
        }
    }

    /// Right-clicks `at`, logs the menu's items and judges them.
    fn menu(&mut self, target: &str, at: (f64, f64), expect_menu: bool) {
        let from = NATIVE.lock().unwrap().len();
        self.input.mouse("rclick", at, &[]);
        thread::sleep(Duration::from_millis(1500));
        let items = items_since(from);
        let kept: Vec<&Item> = items.iter().filter(|item| item.kept).collect();
        eprintln!("CHECK menu {target}: {} offered, {} kept", items.len(), kept.len());
        for item in &items {
            eprintln!("CHECK menu {target}: {}", item.text);
        }
        let outside: Vec<String> =
            kept.iter().filter(|item| !in_fr011_list(&item.text)).map(|i| i.text.clone()).collect();
        for item in outside {
            self.fail(format!("the {target} menu kept an item outside FR-011's list: {item}"));
        }
        if !expect_menu && !kept.is_empty() {
            self.fail(format!("a menu was offered on the {target}, where none belongs"));
        }
        if expect_menu && kept.is_empty() {
            let why = if items.is_empty() {
                "the filter saw no menu (the right click may not have arrived)"
            } else {
                "it kept nothing"
            };
            self.fail(format!("the {target} offered no editing menu: {why}"));
        }
        if !expect_menu && items.is_empty() {
            eprintln!("CHECK note: the {target} gave the filter no menu at all");
        }
        self.input.words(escape());
        thread::sleep(Duration::from_millis(400));
        self.assert_stable(&format!("the {target} menu"));
    }

    /// Types a letter and deletes it: the keyboard still reaches the field.
    fn canary(&mut self, after: &str) {
        let before = self.field();
        self.input.words(letter());
        if !self.field_is(&format!("{before}z"), Duration::from_secs(3)) {
            self.fail(format!(
                "{after} took the keyboard from the field (typed text {:?}, expected {:?})",
                self.field(),
                format!("{before}z")
            ));
            // Back to the field, for the keys after it.
            self.input.mouse("click", FIELD_END, &[]);
            thread::sleep(Duration::from_millis(300));
            return;
        }
        self.input.words(backspace());
        if !self.field_is(&before, Duration::from_secs(3)) {
            self.fail(format!("{after} left the field as {:?}, not {before:?}", self.field()));
        }
    }

    fn run(&mut self) {
        let shared = self.shared.clone();
        let first_load = shared.loads.lock().unwrap().first().cloned().unwrap_or_default();
        self.loads_seen = shared.loads.lock().unwrap().len();
        eprintln!("CHECK first load {first_load:?}");

        // 1. The menus.
        self.menu("blank area", BLANK, false);
        self.menu("row", ROW, false);
        self.menu("button", BUTTON, false);
        self.menu("image", IMAGE, false);
        self.menu("text field", FIELD, true);
        self.eval("selectPara()");
        thread::sleep(Duration::from_millis(300));
        self.menu("selection", PARA, true);

        // 2. Typed input, then every key and the mouse's side buttons.
        self.input.mouse("click", FIELD_END, &[]);
        thread::sleep(Duration::from_millis(300));
        for _ in 0..3 {
            self.input.words(letter());
            thread::sleep(Duration::from_millis(100));
        }
        // The page reports the field every 0.4 s.
        wait_until(Duration::from_secs(3), || self.field().ends_with("zzz"));
        let typed = self.field();
        if !typed.ends_with("zzz") {
            self.fail(format!("typing in the field gave {typed:?}"));
        }
        eprintln!("CHECK typed text {typed:?}");
        for case in KEYS.iter().filter(|case| !case.input().is_empty()) {
            self.input.words(case.input());
            thread::sleep(Duration::from_millis(700));
            eprintln!("CHECK pressed {}", case.name);
            self.assert_stable(case.name);
            self.canary(case.name);
        }
        for (name, direction) in [("mouse back", "back"), ("mouse forward", "forward")] {
            self.input.mouse("xbutton", FIELD, &[direction]);
            thread::sleep(Duration::from_millis(700));
            eprintln!("CHECK pressed {name}");
            self.assert_stable(name);
            self.canary(name);
            // The button click moved focus? Back to the field.
            self.input.mouse("click", FIELD_END, &[]);
            thread::sleep(Duration::from_millis(300));
        }

        // 3. macOS: control-command-D on the selection (Look Up).
        #[cfg(target_os = "macos")]
        self.look_up();

        // The end: the page is the one that was loaded, alive, with the text
        // typed.
        self.assert_stable("the keys and buttons");
        let ticks = shared.ticks.load(Ordering::SeqCst);
        thread::sleep(Duration::from_millis(1200));
        if shared.ticks.load(Ordering::SeqCst) <= ticks {
            self.fail("the page stopped running".into());
        }
        let loads = shared.loads.lock().unwrap().clone();
        if loads.len() != 1 {
            self.fail(format!("the page was loaded {} times: {loads:?}", loads.len()));
        }
        let tick_id = shared.tick.lock().unwrap().clone().map(|(id, _)| id).unwrap_or_default();
        if loads.first().map(|(id, _)| id) != Some(&tick_id) {
            self.fail("the page that is running is not the one first loaded".into());
        }
        if self.field() != typed {
            self.fail(format!("the typed text was lost: {:?} for {typed:?}", self.field()));
        }
    }

    #[cfg(target_os = "macos")]
    fn look_up(&mut self) {
        let pid = std::process::id().to_string();
        let count = |input: &Input| {
            input
                .run(&["windows".into(), pid.clone()])
                .and_then(|out| out.parse::<usize>().ok())
                .unwrap_or(0)
        };
        self.eval("selectPara()");
        thread::sleep(Duration::from_millis(300));
        self.input.mouse("move", PARA, &[]);
        thread::sleep(Duration::from_millis(300));
        let before = count(&self.input);
        let lines = NATIVE.lock().unwrap().len();
        self.input.words("key 2 cmd+ctrl");
        thread::sleep(Duration::from_millis(1800));
        let after = count(&self.input);
        let ignored =
            NATIVE.lock().unwrap().iter().skip(lines).any(|l| l.contains("quickLookWithEvent"));
        eprintln!(
            "CHECK control-command-D: {before} windows before, {after} after; quickLookWithEvent \
             reached the no-op: {ignored}"
        );
        screenshot(&self.options.out.join("look-up-macos.png"));
        if after > before {
            self.fail(format!("control-command-D opened a window ({before} before, {after} after): a Look Up panel?"));
        }
        self.input.words(escape());
        self.assert_stable("control-command-D");
    }
}

fn run(app: AppHandle<Wry>, shared: Arc<Shared>, options: Options) {
    if !wait_until(options.timeout, || !shared.loads.lock().unwrap().is_empty()) {
        eprintln!("CHECK FAILED: the page didn't load");
        screenshot(&options.out.join(format!("failed-{}.png", std::env::consts::OS)));
        std::process::exit(1);
    }
    // Let the window settle on the screen.
    thread::sleep(Duration::from_secs(2));

    // The window's place on the screen, for real input.
    let window = app.get_window("main").unwrap();
    let scale = window.scale_factor().unwrap_or(1.0);
    let inner = window.inner_position().unwrap();
    // macOS: the content view's top is under the title bar; the page starts
    // below it.
    #[cfg(target_os = "macos")]
    let title_bar = hoplodex_lib::services::preview::surface::title_bar_height(
        &app.get_webview("main").unwrap(),
    );
    #[cfg(not(target_os = "macos"))]
    let title_bar = 0.0;
    let input = Input {
        command: options.input.clone(),
        origin: if cfg!(target_os = "macos") {
            (f64::from(inner.x) / scale, f64::from(inner.y) / scale + title_bar)
        } else {
            (f64::from(inner.x), f64::from(inner.y))
        },
        unit: if cfg!(target_os = "macos") { 1.0 } else { scale },
    };
    eprintln!("CHECK window content at {inner:?} scale {scale}");

    let shot = options.out.join(format!("window-controls-{}.png", std::env::consts::OS));
    let mut check = Check {
        app: app.clone(),
        shared,
        options,
        input,
        failures: Vec::new(),
        loads_seen: 0,
        windows_seen: 0,
    };
    // Focus the window: a click on its blank area.
    check.input.mouse("click", BLANK, &[]);
    thread::sleep(Duration::from_millis(500));
    check.run();

    if screenshot(&shot) {
        eprintln!("CHECK screenshot {}", shot.display());
    }
    if check.failures.is_empty() {
        eprintln!("CHECK window controls ok");
        exit_with(&app, 0);
    } else {
        eprintln!("CHECK {} failure(s)", check.failures.len());
        exit_with(&app, 4);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut scratch = None::<PathBuf>;
    let mut out = None::<PathBuf>;
    let mut timeout = Duration::from_secs(40);
    let mut input = Vec::new();
    while let Some(flag) = args.next() {
        let value = args.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--scratch" => scratch = Some(value.into()),
            "--out" => out = Some(value.into()),
            "--timeout" => timeout = Duration::from_secs(value.parse().unwrap_or_else(|_| usage())),
            "--input" => input.push(value),
            _ => usage(),
        }
    }
    if input.is_empty() {
        eprintln!("--input is required: the check is driven with real input");
        usage();
    }
    if !window_controls::INSTALLED {
        eprintln!(
            "this build has debug assertions, so it installs no filters (FR-014): build with \
             --profile e2e or --release"
        );
        std::process::exit(3);
    }
    let temp = scratch.is_none().then(|| tempfile::tempdir().unwrap());
    let scratch = scratch.unwrap_or_else(|| temp.as_ref().unwrap().path().to_path_buf());
    // As the E2E suite isolates an app: its folders, and the main web view's
    // own, under the scratch folder (the names `app_dirs` gives them).
    let cache = scratch.join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(scratch.join("config")).unwrap();
    std::fs::create_dir_all(scratch.join("data")).unwrap();
    #[cfg(windows)]
    let main_dir = cache.join("main-webview");
    let out = out.unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/screenshots-out/window-controls")
    });
    std::fs::create_dir_all(&out).unwrap();

    log::set_logger(&Capture).expect("no logger yet");
    log::set_max_level(log::LevelFilter::Info);

    let shared = Arc::new(Shared::default());
    let options = Mutex::new(Some(Options { out, timeout, input }));
    let mut ctx = tauri::generate_context!();
    ctx.config_mut().app.windows.clear();

    let (serve_shared, setup_shared) = (shared.clone(), shared.clone());
    tauri::Builder::default()
        .register_uri_scheme_protocol(MAIN_SCHEME, move |_ctx, req| serve(&serve_shared, &req))
        .setup(move |app| {
            let base = if cfg!(windows) {
                format!("http://{MAIN_SCHEME}.localhost/")
            } else {
                format!("{MAIN_SCHEME}://localhost/")
            };
            let windows = setup_shared.clone();
            let main =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::External(base.parse().unwrap()))
                    .title("HoploDex window controls check")
                    .inner_size(1000.0, 800.0)
                    .incognito(true)
                    // As the app's: a window is never opened.
                    .on_new_window(move |url, _| {
                        eprintln!("CHECK a new window was asked for: {url}");
                        windows.new_windows.fetch_add(1, Ordering::SeqCst);
                        NewWindowResponse::Deny
                    });
            #[cfg(windows)]
            let main = main.data_directory(main_dir.clone());
            let main = main.build()?;
            // The app's own call, as `setup()` makes it, but logging the items.
            window_controls::apply(&main, true)?;
            let options = options.lock().unwrap().take().expect("setup runs once");
            let (app, shared) = (app.handle().clone(), setup_shared.clone());
            thread::spawn(move || run(app, shared, options));
            Ok(())
        })
        .build(ctx)
        .expect("the check's app failed to build")
        .run(|_, _| {});
    drop(temp);
}
