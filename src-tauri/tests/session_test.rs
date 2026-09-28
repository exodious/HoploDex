//! The session that holds the open database, and the registry of long
//! operations (research.md §13; data-model.md "In memory: the session").

mod support;

use std::cell::Cell;
use std::thread;
use std::time::{Duration, Instant};

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::models::database::OperationKind;
use hoplodex_lib::session::operations::Operations;
use hoplodex_lib::session::{OpenDatabase, Session};
use serde_json::json;
use support::TestDb;

fn count_types(conn: &rusqlite::Connection) -> Result<i64, CommandError> {
    conn.query_row("SELECT count(*) FROM firearm_types", [], |r| r.get(0))
        .map_err(CommandError::from_db)
}

#[test]
fn with_nothing_open_read_and_write_refuse_without_running_the_closure() {
    let session = Session::default();
    let ran = Cell::new(false);

    let read = session.read(|_| {
        ran.set(true);
        Ok(())
    });
    let write = session.write(|_| {
        ran.set(true);
        Ok(())
    });

    assert_eq!(read.unwrap_err().code, "DATABASE_CLOSED");
    assert_eq!(write.unwrap_err().code, "DATABASE_CLOSED");
    assert!(!ran.get());
    assert!(!session.is_open());
}

#[test]
fn an_installed_database_serves_reads_and_writes_until_taken() {
    let db = TestDb::new();
    let path = db.path();
    let (conn, _dir) = db.into_parts();
    let session = Session::default();

    session.install(OpenDatabase::new(conn, &path).unwrap());

    assert!(session.is_open());
    assert_eq!(session.read(count_types).unwrap(), 4);
    session
        .write(|conn| {
            conn.execute("UPDATE collection_settings SET idle_lock_minutes = 30", [])
                .map_err(CommandError::from_db)
        })
        .unwrap();
    let minutes: i64 = session
        .read(|conn| {
            conn.query_row("SELECT idle_lock_minutes FROM collection_settings", [], |r| r.get(0))
                .map_err(CommandError::from_db)
        })
        .unwrap();
    assert_eq!(minutes, 30);

    let taken = session.take().expect("the open database");
    assert_eq!(taken.path, path);
    assert_eq!(taken.name, "test");
    assert_eq!(taken.database_id.len(), 32);
    assert!(!session.is_open());
    assert_eq!(session.read(count_types).unwrap_err().code, "DATABASE_CLOSED");
}

#[test]
fn only_one_long_operation_runs_at_a_time() {
    let operations = Operations::default();
    assert!(!operations.is_running());

    let guard = operations.begin(OperationKind::Import, None).unwrap();
    assert!(operations.is_running());
    let refused = operations.begin(OperationKind::Backup, None).unwrap_err();
    assert_eq!(refused.code, "OPERATION_IN_PROGRESS");
    assert_eq!(refused.details.as_deref(), Some(&json!({ "operation": "import" })));

    drop(guard);
    assert!(!operations.is_running());
    assert!(operations.begin(OperationKind::Backup, None).is_ok());
}

#[test]
fn stopping_with_nothing_running_does_nothing() {
    let operations = Operations::default();
    assert_eq!(operations.stop_running(), None);
    assert!(!operations.is_cancelled());
}

#[test]
fn stop_running_cancels_and_interrupts_a_long_query() {
    let (conn, _dir) = TestDb::new().into_parts();
    let operations = Operations::default();
    let operations = &operations;

    thread::scope(|scope| {
        let worker = scope.spawn(move || {
            let guard = operations
                .begin(OperationKind::PassphraseChange, Some(conn.get_interrupt_handle()))
                .unwrap();
            // Far longer than the test will wait.
            let result = conn.query_row(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n)
                 SELECT count(*) FROM n",
                [],
                |r| r.get::<_, i64>(0),
            );
            (result, guard.is_cancelled())
        });

        let deadline = Instant::now() + Duration::from_secs(10);
        while !operations.is_running() {
            assert!(Instant::now() < deadline, "the operation never registered");
            thread::sleep(Duration::from_millis(5));
        }
        // Give the query a moment to be under way.
        thread::sleep(Duration::from_millis(50));

        assert_eq!(operations.stop_running(), Some(OperationKind::PassphraseChange));
        assert!(operations.is_cancelled());

        let (result, cancelled) = worker.join().unwrap();
        let code = result.unwrap_err().sqlite_error_code();
        assert_eq!(code, Some(rusqlite::ErrorCode::OperationInterrupted));
        assert!(cancelled);
    });
    assert!(!operations.is_running(), "the guard unregisters when dropped");
}
