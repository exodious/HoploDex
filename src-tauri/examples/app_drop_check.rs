// The app drop check (#207): the shipped app itself, given a database with
// one firearm, has a photo and a document dropped onto that firearm's page
// from the OS's file manager, with real input, and then must hold both. The
// app is any build without the `e2e` feature, as a release ships it: the
// installed one, or `cargo build --profile e2e --features custom-protocol`.
// drop_check.rs proves the refusal of the app's own drags on a page of its
// own; this proves what a user does on the real page.
//
//   cargo run --profile e2e --example app_drop_check -- --app BIN \
//       --config DIR --input CMD [--input ARG]... --source CMD [--source ARG]... \
//       [--source-window] [--scratch DIR] [--out DIR] [--timeout SECONDS]
//
// `--config` is the app's own config folder, as the app finds it from the
// environment the caller gives it, which must hold no `machine.json` yet (the
// caller makes sure it isn't anyone's: on Windows, a user's real one). The
// check writes one there listing its database, in the scratch folder, and
// leaves both for the caller to remove.
//
// `--input` is the command that posts real input, given after it:
// `bounds PID` (prints the PID's window as x y width height), `press PID
// NAME` (prints "pressed" once it pressed the PID's control named NAME),
// `drag X1 Y1 X2 Y2`, and on Windows `key KEYS` and `shot FILE`, on macOS
// `type TEXT` and `key CODE`: examples/real_input.ps1 (Windows) or
// real_input.swift (macOS, compiled). `--source` is the command that shows a
// file in the file manager, given `FILE`: it prints the screen point to drag
// the file from, "X Y", keeps the file there while its standard input is
// open, and then puts it away (examples/explorer_source.ps1 on Windows). With
// `--source-window` it is instead one of drop_check.rs's sources, given
// `FILE X Y`: a window of its own at X Y holding the file, until killed.
//
// Exit codes: 0 passed; 1 the database, the app or a step of the way to the
// firearm's page failed; 3 usage; 4 a drop wasn't attached. Everything is
// logged to stderr with a "CHECK" prefix; the app's own log goes to
// `app-drop-<os>-app.log` in `--out`, and its lines about drops to stderr too.

use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::db::{self, Opened};
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;

/// Typed as keys, so nothing in it is a key name (SendKeys braces and the
/// like) on any OS.
const PASSPHRASE: &str = "drop check passphrase 207";
const MAKE: &str = "Dropcheck";
const MODEL: &str = "Pistol";
const PHOTO: &str = "drop-photo.jpg";
const DOCUMENT: &str = "drop-document.pdf";

struct Options {
    app: PathBuf,
    config: PathBuf,
    input: Vec<String>,
    source: Vec<String>,
    source_window: bool,
    scratch: PathBuf,
    out: PathBuf,
    timeout: Duration,
}

fn usage() -> ! {
    eprintln!(
        "usage: app_drop_check --app BIN --config DIR --input CMD [--input ARG]... \
         --source CMD [--source ARG]... [--source-window] [--scratch DIR] [--out DIR] \
         [--timeout SECONDS]"
    );
    std::process::exit(3)
}

fn options() -> (Options, Option<tempfile::TempDir>) {
    let mut args = std::env::args().skip(1);
    let (mut app, mut config, mut scratch, mut out) = (None, None, None, None);
    let mut timeout = Duration::from_secs(40);
    let (mut input, mut source) = (Vec::new(), Vec::new());
    let mut source_window = false;
    while let Some(flag) = args.next() {
        if flag == "--source-window" {
            source_window = true;
            continue;
        }
        let value = args.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--app" => app = Some(PathBuf::from(value)),
            "--config" => config = Some(PathBuf::from(value)),
            "--scratch" => scratch = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--timeout" => timeout = Duration::from_secs(value.parse().unwrap_or_else(|_| usage())),
            "--input" => input.push(value),
            "--source" => source.push(value),
            _ => usage(),
        }
    }
    let (Some(app), Some(config)) = (app, config) else { usage() };
    if input.is_empty() || source.is_empty() {
        eprintln!("--input and --source are required: the check is driven with real input");
        usage();
    }
    let temp = scratch.is_none().then(|| tempfile::tempdir().unwrap());
    let scratch = scratch.unwrap_or_else(|| temp.as_ref().unwrap().path().to_path_buf());
    std::fs::create_dir_all(&scratch).unwrap();
    // The paths a drop names, without Windows' verbatim prefix.
    let scratch = scratch.canonicalize().unwrap();
    let scratch = match scratch.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => scratch,
    };
    let out = out.unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/screenshots-out/drop-check")
    });
    std::fs::create_dir_all(&out).unwrap();
    (Options { app, config, input, source, source_window, scratch, out, timeout }, temp)
}

fn wait_until(limit: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    condition()
}

fn fail(what: &str) -> ! {
    eprintln!("CHECK FAILED: {what}");
    std::process::exit(1)
}

/// The database: one firearm, nothing attached, listed in the app's
/// `machine.json` as the one to open. Returns the firearm's id.
fn make_database(options: &Options, db_path: &Path) -> i64 {
    if options.config.join("machine.json").exists() {
        fail("the config folder already has a machine.json: it may be someone's");
    }
    let machine = MachineSettings::load(&options.config)
        .unwrap_or_else(|err| fail(&format!("machine.json couldn't be made: {err}")));
    let passphrase = Passphrase::from_input(PASSPHRASE.into());
    let conn = db::create_database(db_path, &passphrase, &machine.identity())
        .unwrap_or_else(|err| fail(&format!("the database couldn't be made: {err:?}")));
    let firearm = firearms::create_firearm(&conn, &firearm(), false, None)
        .unwrap_or_else(|err| fail(&format!("the firearm couldn't be made: {err:?}")));
    let id: String = conn
        .query_row("SELECT database_id FROM app_state", [], |row| row.get(0))
        .unwrap_or_else(|err| fail(&format!("no database id: {err}")));
    drop(conn);
    machine.touch_recent(db_path, "Drop check", &id, &options.scratch.join("backups"));
    firearm.id
}

fn firearm() -> FirearmInput {
    FirearmInput {
        make: MAKE.into(),
        model: MODEL.into(),
        serial_number: Some("DC207".into()),
        no_serial_attested: false,
        caliber: "9mm".into(),
        firearm_type_id: 1,
        nickname: None,
        notes: None,
        accessories: None,
        barrel_length_hundredths: None,
        overall_length_hundredths: None,
        weight_tenths_oz: None,
        capacity: None,
        finish: None,
        condition: None,
        status: FirearmStatus::Active,
        estimated_value: None,
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        scheduled_coverage_amount: None,
        origin: None,
        year_of_manufacture: None,
        country_of_manufacture: None,
        importer_name: None,
        original_make: None,
        original_model: None,
        original_serial_number: None,
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        cartridge: None,
        action_type_id: None,
        mounted_on: None,
    }
}

/// Posts real input through the command `--input` names.
struct Input {
    command: Vec<String>,
}

impl Input {
    fn output(&self, args: &[&str]) -> Option<String> {
        let (program, rest) = self.command.split_first().expect("a command");
        match Command::new(program).args(rest).args(args).output() {
            Ok(out) if out.status.success() => Some(String::from_utf8_lossy(&out.stdout).into()),
            other => {
                eprintln!("CHECK input {args:?} failed: {other:?}");
                None
            }
        }
    }

    fn run(&self, args: &[&str]) -> bool {
        self.output(args).is_some()
    }

    fn press(&self, pid: u32, name: &str) -> bool {
        self.output(&["press", &pid.to_string(), name]).is_some_and(|out| out.contains("pressed"))
    }

    /// The PID's window: x y width height.
    fn bounds(&self, pid: u32) -> Option<[i64; 4]> {
        let out = self.output(&["bounds", &pid.to_string()])?;
        let numbers: Vec<i64> = out.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        numbers.try_into().ok()
    }

    /// Types the passphrase into the dialog that has the focus, and presses
    /// Enter.
    fn passphrase(&self) -> bool {
        if cfg!(target_os = "macos") {
            // Return is key code 36.
            self.run(&["type", PASSPHRASE]) && self.run(&["key", "36"])
        } else {
            // SendKeys: the passphrase has no character it reads as a key.
            self.run(&["key", &format!("{PASSPHRASE}{{ENTER}}")])
        }
    }

    /// Saves the whole screen. Best effort.
    fn shot(&self, path: &Path) {
        if cfg!(target_os = "macos") {
            let _ = Command::new("screencapture").arg("-x").arg(path).status();
        } else {
            self.run(&["shot", &path.display().to_string()]);
        }
    }
}

/// Where a `--source-window` source's window goes, in the input command's
/// units, as in drop_check.rs.
const SOURCE_AT: (i64, i64) = (40, 160);

/// A file shown in the file manager, where it can be dragged from.
struct Shown {
    child: Child,
    stdin: Option<ChildStdin>,
    at: (i64, i64),
}

impl Shown {
    fn new(options: &Options, file: &Path) -> Option<Self> {
        let (program, rest) = options.source.split_first().expect("a source command");
        if options.source_window {
            let child = Command::new(program)
                .args(rest)
                .arg(file)
                .args([SOURCE_AT.0.to_string(), SOURCE_AT.1.to_string()])
                .spawn()
                .map_err(|err| eprintln!("CHECK the source window couldn't start: {err}"))
                .ok()?;
            thread::sleep(Duration::from_secs(3));
            return Some(Self { child, stdin: None, at: (SOURCE_AT.0 + 80, SOURCE_AT.1 + 80) });
        }
        let mut child = Command::new(program)
            .args(rest)
            .arg(file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|err| eprintln!("CHECK the file manager couldn't be asked: {err}"))
            .ok()?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take()?;
        let (send, receive) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut line = String::new();
            let _ = BufReader::new(stdout).read_line(&mut line);
            let _ = send.send(line);
        });
        let line = receive.recv_timeout(Duration::from_secs(40)).unwrap_or_default();
        let numbers: Vec<i64> = line.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        let mut shown = Self { child, stdin, at: (0, 0) };
        match numbers.as_slice() {
            [x, y, ..] => {
                shown.at = (*x, *y);
                Some(shown)
            }
            _ => {
                eprintln!("CHECK the file manager didn't show {}", file.display());
                None
            }
        }
    }
}

impl Drop for Shown {
    fn drop(&mut self) {
        // Closing its input tells the source to put the file away; a window
        // source is killed.
        let asked = self.stdin.take().is_some();
        if !asked
            || !wait_until(Duration::from_secs(10), || matches!(self.child.try_wait(), Ok(Some(_))))
        {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// How many photos and documents named `PHOTO` and `DOCUMENT` the firearm
/// has, read once the app is closed.
fn attached(options: &Options, db_path: &Path, firearm: i64) -> (i64, i64) {
    let machine = MachineSettings::load(&options.config).unwrap();
    let passphrase = Passphrase::from_input(PASSPHRASE.into());
    // The app was ended while it had the database open, so its marker for
    // this computer is still set.
    let conn = match db::open_database(db_path, &passphrase, &machine.identity(), true) {
        Ok(Opened::Ready(conn)) => conn,
        Ok(_) => fail("the database wants an upgrade"),
        Err(err) => fail(&format!("the database couldn't be read back: {err:?}")),
    };
    let count = |table: &str, name: &str| -> i64 {
        conn.query_row(
            &format!(
                "SELECT count(*) FROM {table} WHERE firearm_id = ?1 AND original_filename = ?2"
            ),
            rusqlite::params![firearm, name],
            |row| row.get(0),
        )
        .unwrap()
    };
    (count("photos", PHOTO), count("document_attachments", DOCUMENT))
}

fn main() {
    let (options, temp) = options();
    let os = std::env::consts::OS;
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (photo, document) = (options.scratch.join(PHOTO), options.scratch.join(DOCUMENT));
    std::fs::copy(manifest.join("examples/seed-photos/glock-19-gen3.jpg"), &photo).unwrap();
    std::fs::copy(manifest.join("tests/fixtures/documents/three-pages.pdf"), &document).unwrap();
    let db_path = options.scratch.join("drop-check.hoplodex");
    let firearm = make_database(&options, &db_path);
    eprintln!("CHECK the database holds {MAKE} {MODEL}, with nothing attached");

    let app_log = options.out.join(format!("app-drop-{os}-app.log"));
    let log = std::fs::File::create(&app_log).unwrap();
    let mut app = Command::new(&options.app)
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap_or_else(|err| fail(&format!("the app couldn't start: {err}")));
    let pid = app.id();
    let input = Input { command: options.input.clone() };
    let result = drive(&options, &input, pid, &photo, &document);
    input.shot(&options.out.join(format!("app-drop-{os}.png")));
    let _ = app.kill();
    let _ = app.wait();
    // The web view's processes let go of the app's files a moment later.
    thread::sleep(Duration::from_secs(3));
    // Such as a drag refused as a page's.
    let logged = std::fs::read_to_string(&app_log).unwrap_or_default();
    for line in logged.lines().filter(|line| line.contains("drop") || line.contains("drag")) {
        eprintln!("CHECK app: {line}");
    }
    if let Err(what) = result {
        fail(what);
    }

    let (photos, documents) = attached(&options, &db_path, firearm);
    let mut failures = 0;
    for (what, count) in [("photo", photos), ("document", documents)] {
        if count == 1 {
            eprintln!("CHECK the dropped {what} is attached to the firearm");
        } else {
            eprintln!("CHECK FAIL: the dropped {what} is attached {count} times, not once");
            failures += 1;
        }
    }
    drop(temp);
    if failures == 0 {
        eprintln!("CHECK app drops ok");
    } else {
        eprintln!("CHECK {failures} failure(s)");
        std::process::exit(4);
    }
}

/// From the app's start to both drops on the firearm's page.
fn drive(
    options: &Options,
    input: &Input,
    pid: u32,
    photo: &Path,
    document: &Path,
) -> Result<(), &'static str> {
    let mut window = None;
    if !wait_until(options.timeout, || {
        window = input.bounds(pid);
        window.is_some()
    }) {
        return Err("the app's window never showed");
    }
    // The database is the chooser's only one, and selected.
    if !wait_until(options.timeout, || input.press(pid, "Open…")) {
        return Err("the chooser's Open… never showed");
    }
    eprintln!("CHECK pressed Open…");
    thread::sleep(Duration::from_secs(2));
    // macOS types into the frontmost app, which pressing didn't make it.
    if cfg!(target_os = "macos") {
        input.run(&["activate", &pid.to_string()]);
    }
    // The passphrase dialog has the focus, on its field.
    if !input.passphrase() {
        return Err("the passphrase couldn't be typed");
    }
    let name = format!("{MAKE} {MODEL}");
    if !wait_until(options.timeout, || input.press(pid, &name)) {
        return Err("the firearm never showed in the collection");
    }
    eprintln!("CHECK opened {name}'s page");
    thread::sleep(Duration::from_secs(3));
    // The window's right half, clear of the file manager on its left.
    let [x, y, width, height] = window.expect("bounds");
    let target = (x + width * 3 / 4, y + height / 2);
    for file in [photo, document] {
        let Some(shown) = Shown::new(options, file) else {
            return Err("the file manager didn't show a file to drag");
        };
        eprintln!("CHECK drag {} from {:?} to {target:?}", file.display(), shown.at);
        let args = [shown.at.0, shown.at.1, target.0, target.1].map(|v| v.to_string());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        if !input.run(&[&["drag"], args.as_slice()].concat()) {
            return Err("a drag's input couldn't be sent");
        }
        // Attaching reads the file, makes the thumbnail and writes it.
        thread::sleep(Duration::from_secs(4));
    }
    Ok(())
}
