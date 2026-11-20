//! The files and folders the page names reach Rust only as the user gave them
//! (code scanning alerts `paths/cwe-73` on `add_document_from_path` and
//! `export_collection_stoppable`): a drop onto the main window for
//! `add_document_from_path` and `add_photo_from_path`, a chooser answer
//! Rust holds for `import_collection` and `export_collection`. Real
//! databases in a real `Session`, through the commands' own session-level
//! `ops`.

use crate::support;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::db;
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::sandbox::Reach;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use hoplodex_lib::session::grants::{DROP_LIFETIME, PickKind};
use hoplodex_lib::session::{OpenDatabase, Session, SessionId};
use support::{ManualClock, TestDb, firearm, passphrase, sample_png_bytes, test_machine};
use tempfile::TempDir;

const PDF: &[u8] = b"%PDF-1.4 a receipt";

/// A session with a database open and one firearm in it, and a folder of
/// files beside it.
struct World {
    session: Session,
    clock: Arc<ManualClock>,
    db_path: PathBuf,
    owner: RecordRef,
    files: TempDir,
    _db_dir: TempDir,
}

impl World {
    fn new() -> Self {
        let db = TestDb::new();
        let db_path = db.path();
        let firearm =
            firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false, None)
                .unwrap();
        let (conn, dir) = db.into_parts();
        let clock = ManualClock::at("2026-11-19T10:00:00+00:00");
        let session = Session::default().with_clock(clock.clone());
        session.install(OpenDatabase::new(conn, &db_path).unwrap());
        Self {
            session,
            clock,
            db_path,
            owner: RecordRef::Firearm(firearm.id),
            files: tempfile::tempdir().unwrap(),
            _db_dir: dir,
        }
    }

    fn id(&self) -> SessionId {
        self.session.current_id()
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.files.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// Locks and unlocks: the open is dropped and opened again, with a new id.
    fn reopen(&self) {
        drop(self.session.take(self.id()));
        let conn = db::open_database(&self.db_path, &passphrase(), &test_machine(), false)
            .unwrap()
            .expect_ready();
        self.session.install(OpenDatabase::new(conn, &self.db_path).unwrap());
    }

    fn add_document(&self, path: &Path) -> Result<(), CommandError> {
        document_ops::add_dropped_document(
            &self.session,
            self.id(),
            self.owner,
            path,
            &Reach::Unconfined,
        )
        .map(drop)
    }

    fn add_photo(&self, path: &Path) -> Result<(), CommandError> {
        photo_ops::add_dropped_photo(&self.session, self.id(), self.owner, path, &Reach::Unconfined)
            .map(drop)
    }

    fn documents(&self) -> usize {
        self.session
            .read(self.id(), |conn| document_ops::list_documents(conn, self.owner))
            .unwrap()
            .len()
    }

    fn photos(&self) -> usize {
        self.session.read(self.id(), |conn| photo_ops::list_photos(conn, self.owner)).unwrap().len()
    }
}

/// The fixed refusal, which says nothing about the file.
fn refused(result: Result<impl std::fmt::Debug, CommandError>) -> String {
    let err = result.unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR", "{err:?}");
    err.message
}

#[test]
fn a_path_never_dropped_is_refused_whether_or_not_the_file_is_there() {
    let world = World::new();
    let there = world.file("statement.txt", b"account 12345");
    let missing = world.files.path().join("missing.txt");
    let photo = world.file("private.png", &sample_png_bytes());

    let message = refused(world.add_document(&there));
    // The same answer for a file that isn't there: no existence probe.
    assert_eq!(refused(world.add_document(&missing)), message);
    refused(world.add_photo(&photo));
    assert_eq!(world.documents(), 0);
    assert_eq!(world.photos(), 0);
}

#[test]
fn a_dropped_file_is_attached_once() {
    let world = World::new();
    let receipt = world.file("receipt.pdf", PDF);
    let photo = world.file("side.png", &sample_png_bytes());
    world.session.record_drop(vec![receipt.clone(), photo.clone()]);

    world.add_document(&receipt).unwrap();
    world.add_photo(&photo).unwrap();
    assert_eq!((world.documents(), world.photos()), (1, 1));

    refused(world.add_document(&receipt));
    refused(world.add_photo(&photo));
    assert_eq!((world.documents(), world.photos()), (1, 1));
}

#[test]
fn a_drop_is_for_its_kind_of_file_only_once_either_way() {
    // The drop is the user's permission to read the file once, whichever
    // panel takes it.
    let world = World::new();
    let photo = world.file("side.png", &sample_png_bytes());
    world.session.record_drop(vec![photo.clone()]);
    world.add_photo(&photo).unwrap();
    refused(world.add_document(&photo));
}

#[test]
fn a_drop_goes_stale() {
    let world = World::new();
    let receipt = world.file("receipt.pdf", PDF);
    world.session.record_drop(vec![receipt.clone()]);
    world.clock.advance(DROP_LIFETIME + chrono::Duration::seconds(1));
    refused(world.add_document(&receipt));
    assert_eq!(world.documents(), 0);
}

#[test]
fn a_drop_ends_with_its_session() {
    let world = World::new();
    let receipt = world.file("receipt.pdf", PDF);
    world.session.record_drop(vec![receipt.clone()]);
    world.reopen();
    refused(world.add_document(&receipt));
    assert_eq!(world.documents(), 0);
}

#[test]
fn a_drop_with_nothing_open_is_not_kept() {
    let world = World::new();
    let receipt = world.file("receipt.pdf", PDF);
    let id = world.id();
    drop(world.session.take(id));
    world.session.record_drop(vec![receipt.clone()]);
    world.reopen();
    refused(world.add_document(&receipt));
}

#[test]
fn an_earlier_session_cannot_use_a_drop() {
    let world = World::new();
    let earlier = world.id();
    world.reopen();
    let receipt = world.file("receipt.pdf", PDF);
    world.session.record_drop(vec![receipt.clone()]);
    refused(document_ops::add_dropped_document(
        &world.session,
        earlier,
        world.owner,
        &receipt,
        &Reach::Unconfined,
    ));
    // It is still there for the session it was dropped in.
    world.add_document(&receipt).unwrap();
}

/// Exports through the held answer `token`, as `export_collection` does, with
/// an export that succeeds or fails as told; what it was handed, if it ran.
fn export(world: &World, token: &str, succeed: bool) -> Result<PathBuf, CommandError> {
    import_export_ops::export_to_chosen_folder(
        &world.session,
        world.id(),
        token,
        &Reach::Unconfined,
        |folder| {
            if succeed {
                Ok(folder.to_owned())
            } else {
                Err(CommandError::new("OPERATION_STOPPED", "stopped"))
            }
        },
    )
}

#[test]
fn the_export_folder_is_only_a_held_answer() {
    let world = World::new();
    let folder = world.files.path().join("Exports");
    std::fs::create_dir(&folder).unwrap();
    let id = world.id();

    // A path in place of a token, and a token never handed out.
    refused(export(&world, &folder.to_string_lossy(), true));
    refused(export(&world, "0123abcd", true));
    // An answer to the import's question doesn't answer the export's.
    let import = world.session.hold_pick(id, PickKind::ImportFile, folder.clone()).unwrap();
    refused(export(&world, &import, true));

    let token = world.session.hold_pick(id, PickKind::ExportFolder, folder.clone()).unwrap();
    assert_eq!(export(&world, &token, true).unwrap(), folder);
    refused(export(&world, &token, true));
}

#[test]
fn a_failed_export_leaves_its_folder_chosen_and_a_finished_one_uses_it_up() {
    let world = World::new();
    let folder = world.files.path().to_owned();
    let token =
        world.session.hold_pick(world.id(), PickKind::ExportFolder, folder.clone()).unwrap();
    assert_eq!(export(&world, &token, false).unwrap_err().code, "OPERATION_STOPPED");
    assert_eq!(export(&world, &token, true).unwrap(), folder);
    refused(export(&world, &token, true));
}

#[test]
fn the_export_folder_must_still_be_an_absolute_existing_folder() {
    let world = World::new();
    let id = world.id();
    let gone = world.files.path().join("Unplugged");
    let file = world.file("not-a-folder.txt", b"x");
    for path in [gone.clone(), file, PathBuf::from("relative/Exports")] {
        let token = world.session.hold_pick(id, PickKind::ExportFolder, path).unwrap();
        refused(export(&world, &token, true));
    }
    // Nothing was made for the missing one.
    assert!(!gone.exists());
}

#[test]
fn an_export_answer_ends_with_its_session() {
    let world = World::new();
    let folder = world.files.path().to_owned();
    let token = world.session.hold_pick(world.id(), PickKind::ExportFolder, folder).unwrap();
    world.reopen();
    refused(export(&world, &token, true));
}

/// Imports through the held answers `tokens`, as `import_collection` does,
/// with an import that succeeds or fails as told; the files it was handed.
fn import(
    world: &World,
    tokens: &[String],
    succeed: bool,
) -> Result<Vec<(PathBuf, SpreadsheetFormat)>, CommandError> {
    import_export_ops::import_chosen_files(&world.session, world.id(), tokens, |files| {
        if succeed {
            Ok(files.into_iter().map(|f| (f.file_path, f.format)).collect())
        } else {
            Err(CommandError::new("IMPORT_LIMIT_EXCEEDED", "too big"))
        }
    })
}

#[test]
fn the_import_files_are_only_held_answers() {
    let world = World::new();
    let id = world.id();
    let firearms = world.file("firearms.CSV", b"make,model\n");
    let accessories = world.file("accessories.xlsx", b"PK");

    refused(import(&world, &[firearms.to_string_lossy().into_owned()], true));
    let export = world.session.hold_pick(id, PickKind::ExportFolder, firearms.clone()).unwrap();
    refused(import(&world, &[export], true));

    let tokens = vec![
        world.session.hold_pick(id, PickKind::ImportFile, firearms.clone()).unwrap(),
        world.session.hold_pick(id, PickKind::ImportFile, accessories.clone()).unwrap(),
    ];
    assert_eq!(
        import(&world, &tokens, true).unwrap(),
        vec![(firearms, SpreadsheetFormat::Csv), (accessories, SpreadsheetFormat::Xlsx)]
    );
    refused(import(&world, &tokens[..1], true));
}

#[test]
fn a_failed_import_leaves_its_files_chosen_and_a_finished_one_uses_them_up() {
    let world = World::new();
    let id = world.id();
    let path = world.file("firearms.csv", b"make\n");
    let held = world.session.hold_pick(id, PickKind::ImportFile, path.clone()).unwrap();
    assert_eq!(
        import(&world, std::slice::from_ref(&held), false).unwrap_err().code,
        "IMPORT_LIMIT_EXCEEDED"
    );
    // A good token beside a bad one is put back too.
    refused(import(&world, &[held.clone(), "nope".into()], true));
    assert_eq!(import(&world, std::slice::from_ref(&held), true).unwrap().len(), 1);
    refused(import(&world, &[held], true));
}

#[test]
fn an_import_takes_one_or_two_files() {
    let world = World::new();
    let id = world.id();
    refused(import(&world, &[], true));
    let tokens: Vec<_> = (0..3)
        .map(|n| {
            let path = world.file(&format!("{n}.csv"), b"make\n");
            world.session.hold_pick(id, PickKind::ImportFile, path).unwrap()
        })
        .collect();
    refused(import(&world, &tokens, true));
    // Refused before anything was read, and still chosen.
    assert_eq!(import(&world, &tokens[..2], true).unwrap().len(), 2);
}

#[test]
fn an_import_answer_ends_with_its_session() {
    let world = World::new();
    let path = world.file("firearms.csv", b"make\n");
    let token = world.session.hold_pick(world.id(), PickKind::ImportFile, path).unwrap();
    world.reopen();
    refused(import(&world, &[token], true));
}
