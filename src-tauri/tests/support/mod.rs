use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use hoplodex_lib::db;
use hoplodex_lib::models::database::ChooserNotice;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::models::insurance_policy::InsurancePolicyInput;
use hoplodex_lib::services::machine_settings::MachineIdentity;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::spreadsheet::COLUMNS;
use hoplodex_lib::session::clock::Clock;
use hoplodex_lib::session::{Session, SessionEvents};
use rusqlite::Connection;
use tempfile::TempDir;

/// The fixed passphrase of every test database (research.md §20).
#[allow(dead_code)]
pub const TEST_PASSPHRASE: &str = "correct horse battery staple";

/// [`TEST_PASSPHRASE`] as the backend holds it.
#[allow(dead_code)]
pub fn passphrase() -> Passphrase {
    Passphrase::from_input(TEST_PASSPHRASE.to_owned())
}

/// The computer the tests run "on", as the open marker records it.
#[allow(dead_code)]
pub fn test_machine() -> MachineIdentity {
    MachineIdentity { id: "1".repeat(32), display_name: "Test machine".into() }
}

/// Another computer, for open-marker and take-over cases.
#[allow(dead_code)]
pub fn other_machine() -> MachineIdentity {
    MachineIdentity { id: "2".repeat(32), display_name: "Workshop PC".into() }
}

/// A real, migrated, encrypted SQLCipher database in a temp directory —
/// never a mock connection, per the constitution's Testing Standards
/// principle. It is made by the real `db::create_database` with
/// [`TEST_PASSPHRASE`] and the production cipher settings, so every test
/// exercises the real file format. The temp directory lives as long as the
/// handle.
pub struct TestDb {
    pub conn: Connection,
    dir: TempDir,
}

impl TestDb {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("failed to create temp dir for test DB");
        let conn =
            db::create_database(&dir.path().join("test.hoplodex"), &passphrase(), &test_machine())
                .expect("failed to create the test database");
        Self { conn, dir }
    }

    /// The database file.
    #[allow(dead_code)]
    pub fn path(&self) -> PathBuf {
        self.dir.path().join("test.hoplodex")
    }

    /// The temp directory holding the database, for files a test makes
    /// beside it.
    #[allow(dead_code)]
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// Closes the connection and opens the file again, as the app would.
    #[allow(dead_code)]
    pub fn reopen(&mut self) {
        drop(std::mem::replace(&mut self.conn, Connection::open_in_memory().unwrap()));
        self.conn = db::open_database(&self.path(), &passphrase(), &test_machine(), false)
            .expect("failed to reopen the test database");
    }

    /// The connection and the directory that must outlive it, for tests
    /// that hand the connection on (to a session, a thread) or close it.
    #[allow(dead_code)]
    pub fn into_parts(self) -> (Connection, TempDir) {
        (self.conn, self.dir)
    }
}

impl Default for TestDb {
    fn default() -> Self {
        Self::new()
    }
}

/// Called with each event as it is recorded, to act at a given point of an
/// operation (stop a backup midway, look at the files).
type EventHook = Box<dyn Fn(&str, &serde_json::Value) + Send + Sync>;

/// Records the session's events in order, in place of the Tauri app, with
/// each notice kept for the chooser recorded as a `"notice"` event.
#[derive(Default)]
pub struct TestEvents {
    recorded: Mutex<Vec<(String, serde_json::Value)>>,
    hook: Mutex<Option<EventHook>>,
}

impl TestEvents {
    /// Calls `hook` with every later event, before it is recorded.
    #[allow(dead_code)]
    pub fn on_event(&self, hook: impl Fn(&str, &serde_json::Value) + Send + Sync + 'static) {
        *self.hook.lock().unwrap() = Some(Box::new(hook));
    }

    fn record(&self, event: &str, payload: serde_json::Value) {
        if let Some(hook) = &*self.hook.lock().unwrap() {
            hook(event, &payload);
        }
        self.recorded.lock().unwrap().push((event.to_owned(), payload));
    }

    /// The payloads of the recorded `event`s, in order.
    #[allow(dead_code)]
    pub fn payloads(&self, event: &str) -> Vec<serde_json::Value> {
        self.recorded().into_iter().filter(|(name, _)| name == event).map(|(_, p)| p).collect()
    }

    #[allow(dead_code)]
    pub fn recorded(&self) -> Vec<(String, serde_json::Value)> {
        self.recorded.lock().unwrap().clone()
    }

    /// The events recorded so far, which are then forgotten.
    #[allow(dead_code)]
    pub fn take(&self) -> Vec<(String, serde_json::Value)> {
        std::mem::take(&mut *self.recorded.lock().unwrap())
    }
}

impl SessionEvents for TestEvents {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.record(event, payload);
    }

    fn notice(&self, notice: ChooserNotice) {
        self.record("notice", serde_json::to_value(notice).unwrap());
    }
}

/// A clock the test sets and moves, with its own time zone, in place of the
/// computer's.
pub struct ManualClock(Mutex<chrono::DateTime<chrono::FixedOffset>>);

impl ManualClock {
    /// Starts at `rfc3339`, whose offset is the clock's time zone.
    #[allow(dead_code)]
    pub fn at(rfc3339: &str) -> Arc<Self> {
        Arc::new(Self(Mutex::new(chrono::DateTime::parse_from_rfc3339(rfc3339).unwrap())))
    }

    #[allow(dead_code)]
    pub fn set(&self, rfc3339: &str) {
        *self.0.lock().unwrap() = chrono::DateTime::parse_from_rfc3339(rfc3339).unwrap();
    }

    #[allow(dead_code)]
    pub fn advance(&self, by: chrono::Duration) {
        let mut now = self.0.lock().unwrap();
        *now += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> chrono::DateTime<chrono::FixedOffset> {
        *self.0.lock().unwrap()
    }
}

/// An empty session reporting to a [`TestEvents`], with decrypted document
/// copies in `opened_documents`.
#[allow(dead_code)]
pub fn test_session(opened_documents: &Path) -> (Session, Arc<TestEvents>) {
    let events = Arc::new(TestEvents::default());
    let session = Session::new(events.clone(), Some(opened_documents.to_owned()));
    (session, events)
}

/// [`test_session`] on `clock`.
#[allow(dead_code)]
pub fn test_session_at(
    opened_documents: &Path,
    clock: Arc<ManualClock>,
) -> (Session, Arc<TestEvents>) {
    let events = Arc::new(TestEvents::default());
    let session = Session::new(events.clone(), Some(opened_documents.to_owned())).with_clock(clock);
    (session, events)
}

/// Opens the database at `path` with [`TEST_PASSPHRASE`] only to look at
/// it: keyed and configured like the app's connections, but nothing is
/// written, not even the open marker.
#[allow(dead_code)]
pub fn peek(path: &Path) -> Connection {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("could not open the file");
    conn.pragma_update(None, "key", TEST_PASSPHRASE).unwrap();
    db::cipher::apply_cipher_settings(&conn, "main").unwrap();
    conn
}

/// A tiny (20x20, solid red) but genuinely valid PNG, so
/// `services::photos`'s real image-decoding thumbnail generator has real
/// bytes to decode — no mocks, per the constitution.
// Shared by every integration-test crate, but only some of them use each helper.
#[allow(dead_code)]
pub fn sample_png_bytes() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 20, 0, 0, 0, 20, 8,
        2, 0, 0, 0, 2, 235, 138, 90, 0, 0, 0, 26, 73, 68, 65, 84, 120, 218, 99, 248, 207, 192, 64,
        54, 98, 24, 213, 60, 170, 121, 84, 243, 168, 230, 129, 213, 12, 0, 49, 205, 142, 128, 132,
        11, 139, 140, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}

/// One spreadsheet data row built from named cells, in the real export
/// column order (so adding or dropping a column never means hand-editing
/// positional literals). Columns not named are blank; a later entry for the
/// same column wins.
#[allow(dead_code)]
pub fn csv_row(cells: &[(&str, &str)]) -> String {
    for (name, _) in cells {
        assert!(COLUMNS.contains(name), "unknown spreadsheet column {name}");
    }
    COLUMNS
        .iter()
        .map(|column| {
            cells.iter().rev().find(|(name, _)| name == column).map_or("", |(_, value)| value)
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// A complete, valid Handgun row for `make`/`model`/`serial` (a blank
/// serial means "no serial number", attested), with `extra` cells layered
/// on top.
#[allow(dead_code)]
pub fn csv_firearm(make: &str, model: &str, serial: &str, extra: &[(&str, &str)]) -> String {
    let mut cells = vec![
        ("make", make),
        ("model", model),
        ("serial_number", serial),
        ("no_serial_attested", if serial.is_empty() { "TRUE" } else { "FALSE" }),
        ("caliber", "9mm"),
        ("firearm_type", "Handgun"),
        ("estimated_value", "500.00"),
    ];
    cells.extend_from_slice(extra);
    csv_row(&cells)
}

/// The export header plus `rows`, newline-terminated — a whole import file.
#[allow(dead_code)]
pub fn csv_file(rows: &[String]) -> String {
    let mut lines = vec![COLUMNS.join(",")];
    lines.extend_from_slice(rows);
    lines.join("\n") + "\n"
}

/// A valid, active Handgun record for `make`/`model`/`serial`, every other
/// optional field empty — tests set only what they exercise.
#[allow(dead_code)]
pub fn firearm(make: &str, model: &str, serial: &str) -> FirearmInput {
    FirearmInput {
        make: make.into(),
        model: model.into(),
        serial_number: Some(serial.into()),
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
    }
}

/// A valid policy running `start` to `end` (ISO dates); a `limit` makes it
/// a blanket policy, `None` a schedule-only one.
#[allow(dead_code)]
pub fn policy(name: &str, start: &str, end: &str, limit: Option<i64>) -> InsurancePolicyInput {
    InsurancePolicyInput {
        name: name.into(),
        policy_number: format!("{name}-1"),
        insurance_company: "Acme Insurance".into(),
        company_contact: None,
        agent_name: None,
        agent_contact: None,
        notes: None,
        blanket_coverage_limit: limit,
        effective_start_date: start.into(),
        effective_end_date: end.into(),
    }
}

#[allow(dead_code)]
pub fn date(iso: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d").unwrap()
}
