#![cfg(target_os = "linux")]
//! Opening a document in another application through the OpenURI portal, inside
//! a Flatpak (011 US1-7, 011 FR-008; 011 research.md §8; 011 data-model.md "The
//! portal opener"). Each test starts its own `dbus-daemon` on a private address
//! (never the developer's session bus), serves a stand-in
//! `org.freedesktop.portal.OpenURI` on `org.freedesktop.portal.Desktop` with
//! zbus's server side, and hands the portal route a `zbus::Connection` to that
//! bus as a parameter. Nothing goes through an environment variable: the
//! integration program shares one environment.
//!
//! Written ahead of 011 T049; it fails to compile until that exists. Assumed API,
//! the single list to check this file against:
//!
//! - `commands::documents::portal_open(connection: &zbus::Connection, copy:
//!   &Path, timeout: Duration) -> Result<(), OpenFailure>`, `cfg(target_os =
//!   "linux")` and `pub`: the portal route of `AppOpener`, taking the connection
//!   and the time-out (5 s in the app) as parameters. It blocks until the answer,
//!   the time-out or the error, as `Opener::open` does. (`AppOpener::new(route)`
//!   with `OpenerRoute::Portal { timeout }` makes its own session-bus connection
//!   and calls this; that wiring is covered by the E2E Flatpak check, not here.
//!   If the connection ends up inside the route, `open_via_the_portal` and
//!   `PortalOpener` below are the only places to adapt.)
//! - It calls `OpenFile(parent_window = "", fd, options)` with `fd` a read-only
//!   descriptor for the copy and `options["writable"] = false`, and passes
//!   `options["handle_token"]` so the request object's path is known before the
//!   call returns (the portal's documented race, 011 research.md §8): this file
//!   has a test whose `Response` arrives before the call's reply, which only a
//!   subscription made beforehand receives.
//! - `OpenFailure::NoApp` is matched with `matches!`; `OpenFailure` needs no
//!   `PartialEq`.
//! - `open_document`'s signature is unchanged (007), so the route is reached
//!   through the `Opener` trait: `PortalOpener` below adapts `portal_open`.
//!
//! The bus: a minimal `<type>session</type>` configuration written to the test's
//! temp folder, with no service directory, so nothing on the machine (a real
//! `xdg-desktop-portal`) can be started by activation. `dbus-daemon` is in the dev
//! container; a missing one fails the test with a message naming it, rather than
//! skipping it.

use crate::{preview_support, support};

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::documents::{OpenFailure, Opener, portal_open};
use hoplodex_lib::services::consent::{Consent, ConsentAnswer, ConsentRequest};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::session::lifecycle;
use support::{document_fixture, passphrase};
use tempfile::TempDir;
use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue, Value};

/// Long enough that an answer which should arrive does, short enough that a
/// route which ignores it fails the test rather than hanging it.
const WAITING: Duration = Duration::from_secs(10);
/// The time-out a "no answer" test gives the route.
const SHORT: Duration = Duration::from_millis(400);
/// No call that gets an answer, or none to wait for, takes this long.
const PROMPT: Duration = Duration::from_secs(5);

// ------------------------------------------------------------------ the bus

const BUS_CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:dir=@DIR@</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

/// A private `dbus-daemon`, killed when this is dropped.
struct Bus {
    child: Child,
    address: String,
    _stdout: BufReader<ChildStdout>,
    _dir: TempDir,
}

impl Bus {
    fn start() -> Self {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("bus.conf");
        fs::write(&config, BUS_CONFIG.replace("@DIR@", &dir.path().to_string_lossy())).unwrap();
        let mut child = Command::new("dbus-daemon")
            .arg("--config-file")
            .arg(&config)
            .args(["--nofork", "--print-address"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|err| {
                panic!(
                    "this test needs `dbus-daemon` (it is in the dev container, started through \
                     scripts/dev-container.sh), and starting it failed: {err}"
                )
            });
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let address = line.trim().to_owned();
        assert!(!address.is_empty(), "`dbus-daemon` printed no address");
        Self { child, address, _stdout: stdout, _dir: dir }
    }

    /// A connection to this bus, as the app's portal route would make.
    fn connect(&self) -> zbus::Connection {
        zbus::block_on(async {
            zbus::connection::Builder::address(self.address.as_str())?.build().await
        })
        .expect("connecting to the test bus")
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ------------------------------------------------------------------ the portal

/// What the stand-in portal does with `OpenFile`.
#[derive(Clone, Copy)]
enum Answer {
    /// Replies with the request's path, then emits `Response(code)` after
    /// `delay`.
    After { code: u32, delay: Duration },
    /// Emits `Response(code)` and only then replies, so a client that
    /// subscribes after the reply never sees it.
    BeforeReply { code: u32 },
    /// Replies and never answers (the portal is waiting on the user).
    Silent,
    /// Answers the call with a D-Bus error.
    Refuse,
}

/// What the stand-in saw in one `OpenFile` call.
#[derive(Debug)]
struct Seen {
    parent_window: String,
    /// The descriptor's access mode bits (`flags & O_ACCMODE`): 0 is read only.
    access_mode: u32,
    /// The device and inode the descriptor refers to.
    dev_ino: (u64, u64),
    /// What reading the descriptor from its start gave.
    bytes: Vec<u8>,
    writable: Option<bool>,
    option_names: Vec<String>,
}

struct OpenUri {
    answer: Answer,
    seen: Arc<Mutex<Vec<Seen>>>,
}

/// `Response(code, {})` to the caller, on the request object's path.
async fn emit_response(connection: &zbus::Connection, to: &str, handle: &str, code: u32) {
    let results: HashMap<String, Value<'static>> = HashMap::new();
    connection
        .emit_signal(
            Some(to),
            handle,
            "org.freedesktop.portal.Request",
            "Response",
            &(code, results),
        )
        .await
        .expect("emitting the portal's Response");
}

#[zbus::interface(name = "org.freedesktop.portal.OpenURI")]
impl OpenUri {
    async fn open_file(
        &self,
        parent_window: String,
        fd: OwnedFd,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        use std::io::{Read, Seek, SeekFrom};

        let raw = fd.as_raw_fd();
        let info = fs::read_to_string(format!("/proc/self/fdinfo/{raw}")).unwrap_or_default();
        let flags = info
            .lines()
            .find_map(|line| line.strip_prefix("flags:"))
            .and_then(|octal| u32::from_str_radix(octal.trim(), 8).ok())
            .expect("the descriptor's flags");
        let mut file = fs::File::from(std::os::fd::OwnedFd::from(fd));
        let meta = file.metadata().unwrap();
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.read_to_end(&mut bytes).unwrap();
        self.seen.lock().unwrap().push(Seen {
            parent_window,
            access_mode: flags & 0o3,
            dev_ino: (meta.dev(), meta.ino()),
            bytes,
            writable: options.get("writable").and_then(|value| match &**value {
                Value::Bool(writable) => Some(*writable),
                _ => None,
            }),
            option_names: {
                let mut names: Vec<String> = options.keys().cloned().collect();
                names.sort();
                names
            },
        });

        if matches!(self.answer, Answer::Refuse) {
            return Err(zbus::fdo::Error::AccessDenied("the test portal refuses".into()));
        }
        // The request object's path, as the portal documents it: the caller's
        // unique name without the colon and with underscores, then the token.
        let sender = header.sender().map(ToString::to_string).unwrap_or_default();
        let token = options
            .get("handle_token")
            .and_then(|value| match &**value {
                Value::Str(token) => Some(token.to_string()),
                _ => None,
            })
            .unwrap_or_else(|| "no_handle_token".to_owned());
        let handle = format!(
            "/org/freedesktop/portal/desktop/request/{}/{token}",
            sender.trim_start_matches(':').replace('.', "_")
        );
        match self.answer {
            Answer::BeforeReply { code } => emit_response(connection, &sender, &handle, code).await,
            Answer::After { code, delay } => {
                let connection = connection.clone();
                let (to, path) = (sender.clone(), handle.clone());
                std::thread::spawn(move || {
                    std::thread::sleep(delay);
                    zbus::block_on(emit_response(&connection, &to, &path, code));
                });
            }
            Answer::Silent | Answer::Refuse => {}
        }
        Ok(OwnedObjectPath::try_from(handle).expect("a valid object path"))
    }
}

/// A stand-in OpenURI portal on `bus`, kept alive with this value.
struct Portal {
    _connection: zbus::Connection,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Portal {
    fn serve(bus: &Bus, answer: Answer) -> Self {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let service = OpenUri { answer, seen: Arc::clone(&seen) };
        let connection = zbus::block_on(async {
            zbus::connection::Builder::address(bus.address.as_str())?
                .name("org.freedesktop.portal.Desktop")?
                .serve_at("/org/freedesktop/portal/desktop", service)?
                .build()
                .await
        })
        .expect("serving the stand-in portal");
        Self { _connection: connection, seen }
    }

    fn calls(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

// ------------------------------------------------------------------ the route

/// The route under test, on `client`'s bus.
fn open_via_the_portal(
    client: &zbus::Connection,
    copy: &Path,
    timeout: Duration,
) -> Result<(), OpenFailure> {
    portal_open(client, copy, timeout)
}

/// The route as an [`Opener`], for `open_document`.
struct PortalOpener {
    client: zbus::Connection,
    timeout: Duration,
}

impl Opener for PortalOpener {
    fn open(&self, path: &Path) -> Result<(), OpenFailure> {
        open_via_the_portal(&self.client, path, self.timeout)
    }
}

const COPY_BYTES: &[u8] = b"%PDF-1.4 the decrypted copy a test hands to the portal\n";

/// A copy of a document in a folder of its own, as `open_document` makes.
fn a_copy() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let copy = dir.path().join("bill of sale.pdf");
    fs::write(&copy, COPY_BYTES).unwrap();
    (dir, copy)
}

/// The route's answer for a stand-in that behaves as `answer`, and how long it
/// took.
fn ask(answer: Answer, timeout: Duration) -> (Result<(), OpenFailure>, Duration, Portal) {
    let bus = Bus::start();
    let portal = Portal::serve(&bus, answer);
    let client = bus.connect();
    let (_dir, copy) = a_copy();
    let started = Instant::now();
    let result = open_via_the_portal(&client, &copy, timeout);
    let took = started.elapsed();
    drop(bus);
    (result, took, portal)
}

#[track_caller]
fn assert_opened(result: &Result<(), OpenFailure>) {
    assert!(result.is_ok(), "expected Ok(()), got {result:?}");
}

#[track_caller]
fn assert_no_app(result: &Result<(), OpenFailure>) {
    assert!(matches!(result, Err(OpenFailure::NoApp)), "expected Err(NoApp), got {result:?}");
}

// ------------------------------------------------------------------ the table

/// 011 data-model.md "The portal opener": `Response` 0 is opened.
#[test]
fn a_response_of_0_is_opened() {
    let (result, took, portal) =
        ask(Answer::After { code: 0, delay: Duration::from_millis(50) }, WAITING);

    assert_opened(&result);
    assert!(took < PROMPT, "an answer is not waited out: {took:?}");
    assert_eq!(portal.calls(), 1);
}

/// 1 is the user closing the portal's own application chooser: they made the
/// choice, so it counts as opened.
#[test]
fn a_response_of_1_the_user_closing_the_chooser_is_opened() {
    let (result, took, _portal) =
        ask(Answer::After { code: 1, delay: Duration::from_millis(50) }, WAITING);

    assert_opened(&result);
    assert!(took < PROMPT, "{took:?}");
}

/// 2 is the portal being unable to open it, or no application for the type:
/// `NoApp`, which `open_document` turns into `NO_APP_FOR_DOCUMENT`.
#[test]
fn a_response_of_2_is_no_app() {
    let (result, took, _portal) =
        ask(Answer::After { code: 2, delay: Duration::from_millis(50) }, WAITING);

    assert_no_app(&result);
    assert!(took < PROMPT, "{took:?}");
}

/// The portal documents that the `Response` may come before the call returns,
/// so the route subscribes first (011 research.md §8): an answer sent before the
/// reply is still received.
#[test]
fn a_response_sent_before_the_call_returns_is_not_missed() {
    let (result, took, _portal) = ask(Answer::BeforeReply { code: 2 }, WAITING);
    assert_no_app(&result);
    assert!(took < PROMPT, "it waited out the time-out instead: {took:?}");

    let (result, took, _portal) = ask(Answer::BeforeReply { code: 0 }, WAITING);
    assert_opened(&result);
    assert!(took < PROMPT, "{took:?}");
}

/// No answer within the time-out counts as opened: the portal's chooser may be
/// waiting on the user. The time-out is the parameter, not a constant.
#[test]
fn no_answer_within_the_time_out_is_opened_once_it_has_passed() {
    let (result, took, portal) = ask(Answer::Silent, SHORT);

    assert_opened(&result);
    assert!(took >= SHORT, "it did not wait the time-out: {took:?}");
    assert!(took < SHORT + PROMPT, "it waited far past the time-out: {took:?}");
    assert_eq!(portal.calls(), 1);
}

/// No portal service on the bus (no `xdg-desktop-portal` at all) is `NoApp`,
/// and at once, not after the time-out.
#[test]
fn no_portal_on_the_bus_is_no_app_without_waiting() {
    let bus = Bus::start();
    let client = bus.connect();
    let (_dir, copy) = a_copy();
    let started = Instant::now();

    let result = open_via_the_portal(&client, &copy, WAITING);

    assert_no_app(&result);
    assert!(started.elapsed() < PROMPT, "{:?}", started.elapsed());
}

/// The call refused with a D-Bus error is `NoApp`.
#[test]
fn a_call_the_portal_refuses_is_no_app() {
    let (result, took, portal) = ask(Answer::Refuse, WAITING);

    assert_no_app(&result);
    assert!(took < PROMPT, "{took:?}");
    assert_eq!(portal.calls(), 1);
}

// ------------------------------------------------------------------ what is sent

/// 011 FR-008, 011 research.md §8: the portal gets a descriptor opened read
/// only for the copy itself, an empty parent window, and `writable: false`.
#[test]
fn the_portal_is_given_a_read_only_descriptor_for_the_copy_and_writable_false() {
    let bus = Bus::start();
    let portal = Portal::serve(&bus, Answer::After { code: 0, delay: Duration::from_millis(50) });
    let client = bus.connect();
    let (_dir, copy) = a_copy();

    open_via_the_portal(&client, &copy, WAITING).unwrap();

    let seen = portal.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let call = &seen[0];
    assert_eq!(call.parent_window, "", "{call:?}");
    assert_eq!(call.access_mode, 0, "opened read only (O_RDONLY): {call:?}");
    let meta = fs::metadata(&copy).unwrap();
    assert_eq!(call.dev_ino, (meta.dev(), meta.ino()), "the descriptor is the copy: {call:?}");
    assert_eq!(call.bytes, COPY_BYTES);
    assert_eq!(call.writable, Some(false), "{call:?}");
    assert!(call.option_names.contains(&"handle_token".to_owned()), "{call:?}");
    // The route leaves the copy as it found it: `open_document` deletes it.
    assert_eq!(fs::read(&copy).unwrap(), COPY_BYTES);
}

// ------------------------------------------------------------------ through open_document

struct Yes;

impl Consent for Yes {
    fn ask(&self, _request: ConsentRequest) -> ConsentAnswer {
        ConsentAnswer::Open
    }
}

/// An open database with one PDF attached, and the folder `open_document` puts
/// its copies in.
struct World {
    config: TempDir,
    _dir: TempDir,
    session: hoplodex_lib::session::Session,
    machine: MachineSettings,
    document: i64,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let dir = TempDir::new().unwrap();
        let (session, _events) = support::test_session(&config.path().join("opened-documents"));
        let machine = MachineSettings::load(config.path()).unwrap();
        lifecycle::create(&session, &machine, &dir.path().join("Mine.hoplodex"), &passphrase())
            .unwrap();
        let firearm = preview_support::new_firearm(&session);
        let document = preview_support::insert_raw_document(
            &session,
            firearm,
            "bill of sale.pdf",
            "application/pdf",
            &document_fixture("three-pages.pdf"),
        );
        Self { config, _dir: dir, session, machine, document }
    }

    fn copies(&self) -> PathBuf {
        self.config.path().join("opened-documents")
    }

    fn open_document(&self, opener: &dyn Opener) -> Result<bool, CommandError> {
        document_ops::open_document(
            &self.session,
            self.session.current_id(),
            &self.machine,
            &Yes,
            opener,
            &self.copies(),
            self.document,
        )
        .map(|answer| serde_json::to_value(answer).unwrap()["opened"].as_bool().unwrap())
    }

    /// Every file under the copies folder.
    fn copy_files(&self) -> Vec<PathBuf> {
        fn walk(dir: &Path, into: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, into);
                } else {
                    into.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(&self.copies(), &mut files);
        files
    }
}

/// 011 US1-7, 011 FR-008: through the portal route, `open_document` turns the
/// portal's "no application" into `NO_APP_FOR_DOCUMENT` and deletes the copy, as
/// it does for `xdg-open`'s exit status (007 research.md §18).
#[test]
fn open_document_turns_the_portals_no_app_into_no_app_for_document_and_deletes_the_copy() {
    let world = World::new();
    let bus = Bus::start();
    let portal = Portal::serve(&bus, Answer::After { code: 2, delay: Duration::from_millis(50) });
    let opener = PortalOpener { client: bus.connect(), timeout: WAITING };

    let refused = world.open_document(&opener).unwrap_err();

    assert_eq!(refused.code, "NO_APP_FOR_DOCUMENT", "{refused:?}");
    assert!(refused.message.contains("deleted the copy"), "{}", refused.message);
    assert_eq!(portal.calls(), 1, "the portal was asked");
    assert!(world.copy_files().is_empty(), "the copy is deleted: {:?}", world.copy_files());
}

/// 011 US1-7: and when the portal opens it, the document counts as opened and
/// the copy it was given is the document's, in the copies folder.
#[test]
fn open_document_through_the_portal_opens_the_copy_in_the_copies_folder() {
    let world = World::new();
    let bus = Bus::start();
    let portal = Portal::serve(&bus, Answer::After { code: 0, delay: Duration::from_millis(50) });
    let opener = PortalOpener { client: bus.connect(), timeout: WAITING };

    assert!(world.open_document(&opener).unwrap());

    let files = world.copy_files();
    assert_eq!(files.len(), 1, "{files:?}");
    let seen = portal.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let meta = fs::metadata(&files[0]).unwrap();
    assert_eq!(seen[0].dev_ino, (meta.dev(), meta.ino()), "the portal got that copy");
    assert_eq!(seen[0].bytes, document_fixture("three-pages.pdf"));
}

/// 011 US1-7: no portal on the bus is the same refusal, with the copy deleted.
#[test]
fn open_document_with_no_portal_is_no_app_for_document() {
    let world = World::new();
    let bus = Bus::start();
    let opener = PortalOpener { client: bus.connect(), timeout: WAITING };

    let refused = world.open_document(&opener).unwrap_err();

    assert_eq!(refused.code, "NO_APP_FOR_DOCUMENT", "{refused:?}");
    assert!(world.copy_files().is_empty());
}
