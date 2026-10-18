//! Each stored release's database upgrades to this build without losing a value
//! (009 US4-3, 009 FR-023, 009 SC-004, 009 research.md §12). The stored databases
//! are `tests/fixtures/releases/<release>.hoplodex`, made by
//! `examples/release_fixture.rs` and keyed with the pinned KDF, so this is a
//! test program of its own (`Cargo.toml`): every test here keys with the pinned
//! KDF, in a `test-kdf` build too (`support::one_at_a_time_with_the_real_kdf`),
//! and that applies to a whole process.
//!
//! Select it with `-E 'binary(release_upgrade_test)'` or
//! `--test release_upgrade_test`.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::APP_VERSION;
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::db::cipher::CIPHER_SETTINGS;
use hoplodex_lib::db::upgrade::testing;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::record_id;
use hoplodex_lib::session::{Session, lifecycle};
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use support::{ManualClock, TestDb, one_at_a_time_with_the_real_kdf as one_at_a_time};
use tempfile::TempDir;

/// A stored release database: its release, file and passphrase
/// (`examples/release_fixture.rs`). A later release adds its own.
struct Release {
    version: &'static str,
    file: &'static str,
    passphrase: &'static str,
}

const RELEASES: &[Release] =
    &[Release { version: "0.1.0", file: "0.1.0.hoplodex", passphrase: "release fixture 0.1.0" }];

fn fixture_path(release: &Release) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/releases").join(release.file)
}

/// `path` read with a raw keyed connection, read-only: the key and the pinned
/// cipher settings (whatever this build's KDF, since a shipped build keyed the
/// fixture), and no upgrade step run.
fn raw_connection(path: &Path, passphrase: &str) -> Connection {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap();
    conn.pragma_update(None, "key", passphrase).unwrap();
    for (name, value) in CIPHER_SETTINGS {
        conn.pragma_update(None, name, value).unwrap();
    }
    conn
}

/// A copy of the release's database in a temp directory, read raw.
fn raw_fixture() -> (TempDir, Connection) {
    let release = &RELEASES[0];
    let dir = TempDir::new().unwrap();
    let copy = dir.path().join(release.file);
    fs::copy(fixture_path(release), &copy).expect("the stored 0.1.0 database");
    let conn = raw_connection(&copy, release.passphrase);
    (dir, conn)
}

/// Every object in `sqlite_schema` as `type name: sql`, in a fixed order.
fn schema(conn: &Connection) -> Vec<String> {
    conn.prepare(
        "SELECT type || ' ' || name || ': ' || coalesce(sql, '') FROM sqlite_schema
         ORDER BY type, name",
    )
    .unwrap()
    .query_map([], |row| row.get(0))
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn migration_names(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT name FROM schema_migrations ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// 009 FR-023, research.md §12: until 0.1.0 is released, the layout still
/// changes in place (CLAUDE.md), so the stored 0.1.0 database has to be what
/// a new database is. From the release on it is never regenerated, and this
/// check stops applying to it.
#[test]
fn release_fixture_matches_layout() {
    let _serial = one_at_a_time();
    let (_dir, fixture) = raw_fixture();
    let new = TestDb::new();

    let (stored, current) = (schema(&fixture), schema(&new.conn));
    let hint = "src-tauri/tests/fixtures/releases/0.1.0.hoplodex is out of step with the layout a \
                new database has (until 0.1.0 is released the layout changes in place); rerun \
                `cargo run --example release_fixture` and commit the result";
    assert!(stored.len() > 20, "the fixture's schema was not read");
    assert_eq!(stored, current, "{hint}: sqlite_schema differs");
    assert_eq!(
        migration_names(&fixture),
        migration_names(&new.conn),
        "{hint}: schema_migrations differs"
    );
}

// ---------------------------------------------------------------------------
// A picture of a database that two copies can be compared by
// ---------------------------------------------------------------------------

/// One cell: a blob by its SHA-256, so a photo's bytes are compared without
/// being held.
#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(String),
}

impl Cell {
    fn is_null(&self) -> bool {
        matches!(self, Cell::Null)
    }
}

impl From<Value> for Cell {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => Cell::Null,
            Value::Integer(n) => Cell::Integer(n),
            Value::Real(x) => Cell::Real(x),
            Value::Text(text) => Cell::Text(text),
            Value::Blob(bytes) => Cell::Blob(format!("{:x}", Sha256::digest(bytes))),
        }
    }
}

/// A row by column name.
type Row = BTreeMap<String, Cell>;
/// Every row of every table, by table name.
type Tables = BTreeMap<String, Vec<Row>>;

/// Every ordinary table's rows, and the shadow tables a full-text index keeps
/// its words in (the virtual tables are external-content and cannot be read as
/// a table), each ordered by what is in it.
fn read_tables(conn: &Connection) -> Tables {
    let names: Vec<String> = conn
        .prepare(
            "SELECT name FROM pragma_table_list
             WHERE schema = 'main' AND type IN ('table', 'shadow') AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut tables = Tables::new();
    for name in names {
        let mut stmt = conn.prepare(&format!("SELECT * FROM \"{name}\"")).unwrap();
        let columns: Vec<String> = stmt.column_names().into_iter().map(str::to_owned).collect();
        let mut rows: Vec<Row> = stmt
            .query_map([], |row| {
                columns
                    .iter()
                    .enumerate()
                    .map(|(i, column)| Ok((column.clone(), Cell::from(row.get::<_, Value>(i)?))))
                    .collect()
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        // Ordered by what is in them, so two copies compare whatever order
        // SQLite returned the rows in.
        rows.sort_by_key(|row| format!("{row:?}"));
        tables.insert(name, rows);
    }
    tables
}

/// What `app_state` holds that the open itself writes (009 research.md §4):
/// the open marker, which an upgrade sets for the computer that upgrades.
const MARKER_COLUMNS: &[&str] = &["open_machine_id", "open_machine_name", "open_since"];

/// A database as the upgrade must leave it: the collection and its settings
/// apart from `app_state`, which is split into its marker fields and the rest,
/// the pending changes, and the applied steps.
#[derive(Debug, Clone, PartialEq)]
struct Picture {
    /// Every table but the three below.
    tables: Tables,
    /// `app_state` without the open marker.
    app_state: Row,
    marker: Row,
    pending: Vec<Row>,
    /// `schema_migrations` as (name, applied_at), by name.
    steps: Vec<(String, Cell)>,
}

fn picture(conn: &Connection) -> Picture {
    let mut tables = read_tables(conn);
    let mut take = |name: &str| tables.remove(name).unwrap_or_else(|| panic!("no table {name}"));
    let app_state = take("app_state");
    assert_eq!(app_state.len(), 1, "app_state has one row");
    let (marker, app_state): (Row, Row) = app_state
        .into_iter()
        .next()
        .unwrap()
        .into_iter()
        .partition(|(column, _)| MARKER_COLUMNS.contains(&column.as_str()));
    let pending = take("pending_changes");
    let mut steps: Vec<(String, Cell)> = take("schema_migrations")
        .into_iter()
        .map(|mut row| {
            let Some(Cell::Text(name)) = row.remove("name") else { panic!("a step has a name") };
            (name, row.remove("applied_at").unwrap())
        })
        .collect();
    steps.sort_by(|a, b| a.0.cmp(&b.0));
    Picture { tables, app_state, marker, pending, steps }
}

fn step_names(picture: &Picture) -> Vec<&str> {
    picture.steps.iter().map(|(name, _)| name.as_str()).collect()
}

/// The columns a closed database that is not a backup always has NULL, which
/// the "holds every kind" check lets be (009 FR-023): `app_state`'s open
/// marker (no computer holds a closed file) and its backup stamp (only a backup
/// has one, and the stamp is checked on the backup instead), and
/// `pending_changes.target_id` (the fixture keeps the draft of an added policy,
/// which has no record yet; an edit draft would fill it, when the fixture is
/// next regenerated).
const ALWAYS_NULL: &[(&str, &str)] = &[
    ("app_state", "open_machine_id"),
    ("app_state", "open_machine_name"),
    ("app_state", "open_since"),
    ("app_state", "backup_made_at"),
    ("app_state", "backup_of_name"),
    ("app_state", "backup_before_upgrade_to"),
    ("pending_changes", "target_id"),
];

/// 009 FR-023, 009 research.md §12: every table of a stored release database
/// has a row, and every column a value that is not NULL somewhere, so "it
/// holds every kind of record" cannot erode as the layout grows.
#[test]
fn every_stored_release_database_holds_every_kind() {
    let _serial = one_at_a_time();
    for release in RELEASES {
        let dir = TempDir::new().unwrap();
        let copy = dir.path().join(release.file);
        fs::copy(fixture_path(release), &copy).unwrap();
        let conn = raw_connection(&copy, release.passphrase);
        let tables = read_tables(&conn);
        assert!(tables.len() > 10, "{}: the tables were not read", release.version);
        let mut missing = Vec::new();
        for (table, rows) in tables {
            if rows.is_empty() {
                missing.push(format!("{table} has no row"));
                continue;
            }
            for column in rows[0].keys() {
                let exempt = ALWAYS_NULL.contains(&(table.as_str(), column.as_str()));
                let has_value = rows.iter().any(|row| !row[column].is_null());
                if !has_value && !exempt {
                    missing.push(format!("{table}.{column} is NULL in every row"));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "the {} database must hold a row in every table and a value in every column; \
             regenerate it with `cargo run --example release_fixture` (before the 0.1.0 tag) or \
             make the next release's fixture richer: {missing:#?}",
            release.version
        );
    }
}

// ---------------------------------------------------------------------------
// The upgrade
// ---------------------------------------------------------------------------

const START: &str = "2026-10-01T10:00:00+00:00";

/// A session, a clock and the computer's settings in temp directories, and a
/// folder for the database being upgraded.
struct World {
    dir: TempDir,
    _config: TempDir,
    machine: Arc<MachineSettings>,
    session: Session,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let clock = ManualClock::at(START);
        let (session, _events) =
            support::test_session_at(&config.path().join("opened-documents"), clock);
        Self {
            dir: TempDir::new().unwrap(),
            machine: Arc::new(MachineSettings::load(config.path()).unwrap()),
            _config: config,
            session,
        }
    }

    /// A copy of the release's database, as `Older.hoplodex`.
    fn copy_of(&self, release: &Release) -> PathBuf {
        let path = self.dir.path().join("Older.hoplodex");
        fs::copy(fixture_path(release), &path).unwrap();
        path
    }

    fn backups_folder(&self) -> PathBuf {
        self.dir.path().join("HoploDex backups")
    }

    fn open(&self, path: &Path, release: &Release) -> Result<(), CommandError> {
        let passphrase = Passphrase::from_input(release.passphrase.to_owned());
        lifecycle::open(&self.session, &self.machine, path, &passphrase, false)
    }

    /// The open database's picture.
    fn open_picture(&self) -> Picture {
        self.session.inspect(self.session.current_id(), |open| Ok(picture(&open.conn))).unwrap()
    }
}

fn awaiting_id(err: &CommandError) -> u64 {
    assert_eq!(err.code, "UPGRADE_NEEDED", "{err:?}");
    err.details
        .as_deref()
        .and_then(|details| details["awaitingId"].as_u64())
        .expect("UPGRADE_NEEDED carries an awaitingId")
}

/// The test-only steps every test program shares (009 research.md §14): there
/// is no step after 0.1.0 yet, and these stand in for the kinds a later
/// release will have.
fn test_steps() -> [(&'static str, &'static str); 2] {
    [testing::PHOTOS_REBUILD, testing::HISTORY_UID_FILL]
}

/// What each step is declared to do to the rows (009 FR-023, research.md §12),
/// applied to the `expected` picture (the copy's, before the upgrade) to make
/// what the upgraded database must hold. `actual` is read only where a step
/// writes values the test cannot know (a random identifier). A step without an
/// entry fails the test: add one, an empty one if the step changes no row.
fn declared_change(step: &str, expected: &mut Picture, actual: &Picture) {
    match step {
        // Released steps go here, each with the change it makes to the rows
        // (none yet: 0001 to 0003 are what the fixture was made with).
        //
        // A rebuild of `photos` that changes a CHECK and recreates the
        // indexes and triggers changes no row.
        name if name == testing::PHOTOS_REBUILD.0 => {}
        // A new required unique value on `disposition_history`: every row gets
        // a distinct valid identifier, which the test cannot know.
        name if name == testing::HISTORY_UID_FILL.0 => {
            let rows = expected.tables.get_mut("disposition_history").unwrap();
            let uids: Vec<Cell> =
                actual.tables["disposition_history"].iter().map(|row| row["uid"].clone()).collect();
            assert_eq!(uids.len(), rows.len(), "a history row was added or lost");
            let mut seen = Vec::new();
            for uid in &uids {
                let Cell::Text(text) = uid else { panic!("a history row has no uid: {uid:?}") };
                assert_eq!(record_id::parse(text).as_deref(), Some(text.as_str()), "{text}");
                assert!(!seen.contains(&text), "{text} is used twice");
                seen.push(text);
            }
            // Rows are ordered by their content, which the new column changes:
            // pair them up by id.
            let mut by_id: BTreeMap<i64, Cell> = BTreeMap::new();
            for row in &actual.tables["disposition_history"] {
                let Cell::Integer(id) = row["id"] else { panic!("a history row has no id") };
                by_id.insert(id, row["uid"].clone());
            }
            for row in rows.iter_mut() {
                let Cell::Integer(id) = row["id"] else { panic!("a history row has no id") };
                row.insert("uid".into(), by_id[&id].clone());
            }
        }
        other => panic!(
            "step {other} has no declared change in tests/release_upgrade_test.rs \
             (declared_change): say what it does to the rows, or that it changes none (009 FR-023)"
        ),
    }
}

/// The names in `folder` that are backups made before an upgrade.
fn before_upgrade_backups(folder: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(folder)
        .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
        .unwrap_or_default();
    found.retain(|path| {
        let name = path.file_name().unwrap().to_string_lossy();
        name.contains(" before ") && name.ends_with(".hoplodex")
    });
    found.sort();
    found
}

/// 009 US4-3, 009 FR-023, 009 SC-004, 009 research.md §12: a copy of each
/// stored release database is read as it is; another copy is opened and
/// upgraded as the app does (`lifecycle::open`, then `upgrade_database`), under
/// the shared test-only steps. The backup before the upgrade is at the backup
/// location with its ` before ` name and stamp, and its collection rows equal
/// the first copy's; the upgraded rows equal the first copy's after each step's
/// declared change; and every record keeps its identifier (009 US1-3).
#[test]
fn a_stored_release_upgrades_with_every_value_and_a_backup_first() {
    let _serial = one_at_a_time();
    for release in RELEASES {
        let world = World::new();
        let (before, raw_stamp) = {
            let dir = TempDir::new().unwrap();
            let copy = dir.path().join(release.file);
            fs::copy(fixture_path(release), &copy).unwrap();
            let conn = raw_connection(&copy, release.passphrase);
            (picture(&conn), conn_stamp(&conn))
        };
        assert!(!before.pending.is_empty(), "the fixture keeps pending changes apart");

        let path = world.copy_of(release);
        let _scope = testing::scope(world.dir.path()).extra_steps(&test_steps());
        let refused = world.open(&path, release).expect_err("steps are due, so it must ask");
        let id = awaiting_id(&refused);
        assert!(!world.backups_folder().exists(), "the question made a backup");

        databases::upgrade_database(&world.session, &world.machine, id, None)
            .unwrap_or_else(|err| panic!("{}: the upgrade failed: {err:?}", release.version));
        assert!(world.session.is_open());

        // The backup first, at the backup location.
        let backups = before_upgrade_backups(&world.backups_folder());
        assert_eq!(backups.len(), 1, "{}: {backups:?}", release.version);
        let name = backups[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("Older ")
                && name.ends_with(&format!(" before {APP_VERSION}.hoplodex")),
            "{name}"
        );
        let backup = picture(&raw_connection(&backups[0], release.passphrase));
        assert_eq!(backup.tables, before.tables, "{}: the backup's rows differ", release.version);
        assert_eq!(step_names(&backup), step_names(&before), "the backup was made after a step");
        assert!(backup.pending.is_empty(), "the stamp empties the pending changes");
        assert_eq!(backup.marker.values().cloned().collect::<Vec<_>>(), vec![Cell::Null; 3]);
        let stamp = |column: &str| backup.app_state[column].clone();
        assert_eq!(stamp("backup_before_upgrade_to"), Cell::Text(APP_VERSION.to_owned()));
        assert!(!stamp("backup_made_at").is_null() && !stamp("backup_of_name").is_null());
        assert_eq!(stamp("database_id"), before.app_state["database_id"]);
        assert_eq!(stamp("changes_waiting"), Cell::Integer(0));
        assert_eq!(raw_stamp, vec![Cell::Null; 3], "the stored database carries no backup stamp");

        // The upgraded database: every row, after each step's declared change.
        let upgraded = world.open_picture();
        let new_steps: Vec<&str> = step_names(&upgraded)
            .into_iter()
            .filter(|name| !step_names(&before).contains(name))
            .collect();
        assert_eq!(
            new_steps,
            test_steps().iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            "{}: the steps this build applied to the stored database",
            release.version
        );
        let mut expected = before.clone();
        for step in &new_steps {
            declared_change(step, &mut expected, &upgraded);
        }
        assert_eq!(upgraded.tables, expected.tables, "{}: a row changed", release.version);
        // Everything but the steps' own rows and the open marker is kept:
        // the identity, the settings, `changes_waiting`, the saved version.
        assert_eq!(upgraded.app_state, expected.app_state, "app_state changed");
        assert_eq!(upgraded.pending, expected.pending, "the pending changes changed");
        assert_eq!(
            upgraded.steps[..before.steps.len().min(upgraded.steps.len())]
                .iter()
                .filter(|(name, _)| step_names(&before).contains(&name.as_str()))
                .cloned()
                .collect::<Vec<_>>(),
            before.steps,
            "an applied step's row changed"
        );
        // The computer that upgraded holds the file now.
        assert_eq!(
            upgraded.marker["open_machine_id"],
            Cell::Text(world.machine.identity().id),
            "the open marker"
        );

        // Identifiers survive (009 US1-3).
        for table in
            ["firearms", "accessories", "insurance_policies", "photos", "document_attachments"]
        {
            let uids = |picture: &Picture| -> Vec<(Cell, Cell)> {
                picture.tables[table]
                    .iter()
                    .map(|row| (row["id"].clone(), row["uid"].clone()))
                    .collect()
            };
            assert!(!uids(&before).is_empty(), "{table} has no row in the fixture");
            assert_eq!(uids(&upgraded), uids(&before), "{table}: a uid changed");
        }
    }
}

/// The backup stamp fields of the stored database.
fn conn_stamp(conn: &Connection) -> Vec<Cell> {
    ["backup_made_at", "backup_of_name", "backup_before_upgrade_to"]
        .iter()
        .map(|column| {
            conn.query_row(&format!("SELECT {column} FROM app_state"), [], |row| {
                row.get::<_, Value>(0)
            })
            .map(Cell::from)
            .unwrap()
        })
        .collect()
}

/// 009 US4-3, 009 research.md §12: with no step due, as today for 0.1.0's
/// database, the stored database opens at once, with no question and no backup,
/// and keeps every row.
#[test]
fn a_stored_release_with_nothing_to_upgrade_opens_unchanged() {
    let _serial = one_at_a_time();
    for release in RELEASES {
        let world = World::new();
        let before = {
            let dir = TempDir::new().unwrap();
            let copy = dir.path().join(release.file);
            fs::copy(fixture_path(release), &copy).unwrap();
            picture(&raw_connection(&copy, release.passphrase))
        };
        let path = world.copy_of(release);
        world.open(&path, release).expect("nothing is due, so it opens");
        assert!(world.session.is_open());
        assert!(!world.backups_folder().exists(), "an open with nothing due made a backup");
        let opened = world.open_picture();
        assert_eq!(opened.tables, before.tables);
        assert_eq!(opened.app_state, before.app_state);
        assert_eq!(opened.pending, before.pending);
        assert_eq!(opened.steps, before.steps);
    }
}
