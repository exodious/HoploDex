//! Constitution V: deleting a firearm, photo or document must actually remove
//! its content, not just hide it. SQLite normally leaves deleted content in
//! the file (in place on its page, then on the free list); with the database
//! encrypted that is still recoverable by anyone holding the key, so deletion
//! zeroes freed pages (`secure_delete`) and vacuums the file back down.
//!
//! Run against a real temporary SQLCipher database — no mocks.

mod support;

use std::path::Path;

use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::photos::ops as photo_ops;
use rusqlite::Connection;
use support::{TestDb, firearm, sample_png_bytes};

const MARKER: &[u8] = b"HOPLODEX-WIPE-ME-0123456789ABCDEF";
/// Big enough to span many pages, so a leak can't hide in one.
const REPEATS: usize = 8_000;

fn marker_blob() -> Vec<u8> {
    MARKER.repeat(REPEATS)
}

/// A real PNG (so the thumbnail generator accepts it) with the marker
/// appended after its final chunk, where decoders ignore it.
fn marker_png() -> Vec<u8> {
    let mut bytes = sample_png_bytes();
    bytes.extend(marker_blob());
    bytes
}

fn count_marker(bytes: &[u8]) -> usize {
    bytes.windows(MARKER.len()).filter(|window| *window == MARKER).count()
}

/// Everything the database holds, decrypted, as a plaintext file's bytes.
fn decrypted_export(conn: &Connection, dir: &Path) -> Vec<u8> {
    let path = dir.join("decrypted-export.db");
    let _ = std::fs::remove_file(&path);
    conn.execute("ATTACH DATABASE ?1 AS export KEY ''", [path.to_str().unwrap()]).unwrap();
    conn.query_row("SELECT sqlcipher_export('export')", [], |_| Ok(())).unwrap();
    conn.execute("DETACH DATABASE export", []).unwrap();
    std::fs::read(path).unwrap()
}

fn file_size(conn: &Connection) -> u64 {
    std::fs::metadata(conn.path().unwrap()).unwrap().len()
}

fn freelist_count(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA freelist_count", [], |row| row.get(0)).unwrap()
}

/// Asserts the deleted marker is gone from the decrypted contents, that no
/// free pages remain to hold a remnant, and that the file gave the space
/// back.
fn assert_wiped(conn: &Connection, size_before_delete: u64, scratch: &Path) {
    assert_eq!(
        count_marker(&decrypted_export(conn, scratch)),
        0,
        "deleted content is still in the database"
    );
    assert_eq!(freelist_count(conn), 0, "freed pages were left in the file");
    let size_after = file_size(conn);
    let blob = (MARKER.len() * REPEATS) as u64;
    assert!(
        size_after + blob / 2 < size_before_delete,
        "the file kept the freed space: {size_before_delete} bytes before, {size_after} after"
    );
}

#[test]
fn the_search_can_see_the_marker_while_it_is_stored() {
    // Guards the other tests: a search that finds nothing proves nothing
    // unless it finds the marker when it is there.
    let db = TestDb::new();
    let scratch = tempfile::TempDir::new().unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "W-0"), false).unwrap();
    document_ops::add_document(
        &db.conn,
        created.id,
        &marker_blob(),
        "receipt.pdf",
        "application/pdf",
    )
    .unwrap();

    // Markers that straddle a page boundary are split, so expect most, not all.
    assert!(count_marker(&decrypted_export(&db.conn, scratch.path())) > REPEATS / 2);
}

#[test]
fn every_connection_zeroes_deleted_content() {
    let db = TestDb::new();
    let mode: i64 = db.conn.query_row("PRAGMA secure_delete", [], |row| row.get(0)).unwrap();
    assert_eq!(mode, 1, "PRAGMA secure_delete must be ON right after the key is applied");
}

#[test]
fn deleting_a_document_wipes_its_bytes_and_returns_the_space() {
    let db = TestDb::new();
    let scratch = tempfile::TempDir::new().unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "W-1"), false).unwrap();
    let document = document_ops::add_document(
        &db.conn,
        created.id,
        &marker_blob(),
        "receipt.pdf",
        "application/pdf",
    )
    .unwrap();
    let size_before = file_size(&db.conn);

    document_ops::delete_document(&db.conn, document.id, true).unwrap();

    assert_wiped(&db.conn, size_before, scratch.path());
}

#[test]
fn deleting_a_photo_wipes_its_bytes_and_returns_the_space() {
    let db = TestDb::new();
    let scratch = tempfile::TempDir::new().unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "W-2"), false).unwrap();
    let photo = photo_ops::add_photo(&db.conn, created.id, &marker_png(), "range.png", "image/png")
        .unwrap();
    let size_before = file_size(&db.conn);

    photo_ops::delete_photo(&db.conn, photo.id, true).unwrap();

    assert_wiped(&db.conn, size_before, scratch.path());
}

#[test]
fn deleting_a_firearm_wipes_its_photos_and_documents_too() {
    let db = TestDb::new();
    let scratch = tempfile::TempDir::new().unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "W-3"), false).unwrap();
    photo_ops::add_photo(&db.conn, created.id, &marker_png(), "range.png", "image/png").unwrap();
    document_ops::add_document(
        &db.conn,
        created.id,
        &marker_blob(),
        "receipt.pdf",
        "application/pdf",
    )
    .unwrap();
    let size_before = file_size(&db.conn);

    firearm_ops::delete_firearm(&db.conn, created.id, true).unwrap();

    assert_wiped(&db.conn, size_before, scratch.path());
}

/// specs/004-cartridges-action-types SC-005 / research.md §14: a custom
/// cartridge used by one firearm is gone from the file, its index entry
/// included, once that firearm is deleted.
#[test]
fn deleting_a_firearm_wipes_its_custom_cartridge() {
    const CARTRIDGE: &str = "Zyxwv Wildcat Special";
    let contains = |haystack: &[u8], needle: &str| {
        haystack.windows(needle.len()).any(|window| window == needle.as_bytes())
    };
    let db = TestDb::new();
    let scratch = tempfile::TempDir::new().unwrap();
    let created = firearm_ops::create_firearm(
        &db.conn,
        &hoplodex_lib::models::firearm::FirearmInput {
            cartridge: Some(CARTRIDGE.into()),
            caliber: ".30".into(),
            ..firearm("Glock", "19", "W-6")
        },
        false,
    )
    .unwrap();
    firearm_ops::create_firearm(&db.conn, &firearm("Ruger", "LCP", "W-7"), false).unwrap();
    // Guard: the search sees the text and its index token while stored.
    let before = decrypted_export(&db.conn, scratch.path());
    assert!(contains(&before, CARTRIDGE));
    assert!(contains(&before, "zyxwv"), "the full-text index holds the word");

    firearm_ops::delete_firearm(&db.conn, created.id, true).unwrap();

    let after = decrypted_export(&db.conn, scratch.path());
    assert!(!contains(&after, CARTRIDGE), "the cartridge is still in the database");
    assert!(!contains(&after, "zyxwv") && !contains(&after, "Zyxwv"), "a remnant was left");
    assert_eq!(freelist_count(&db.conn), 0, "freed pages were left in the file");
}

#[test]
fn deleting_one_attachment_leaves_the_others_intact() {
    let db = TestDb::new();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "W-4"), false).unwrap();
    let keep = document_ops::add_document(
        &db.conn,
        created.id,
        &marker_blob(),
        "keep.pdf",
        "application/pdf",
    )
    .unwrap();
    let doomed = document_ops::add_document(
        &db.conn,
        created.id,
        b"%PDF-1.4 to be deleted",
        "gone.pdf",
        "application/pdf",
    )
    .unwrap();

    document_ops::delete_document(&db.conn, doomed.id, true).unwrap();

    let kept = document_ops::get_document(&db.conn, keep.id).unwrap();
    assert_eq!(kept.file_bytes, marker_blob(), "the vacuum must not disturb what stays");
}

#[test]
fn an_unconfirmed_delete_changes_nothing() {
    let db = TestDb::new();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "W-5"), false).unwrap();
    let document = document_ops::add_document(
        &db.conn,
        created.id,
        &marker_blob(),
        "receipt.pdf",
        "application/pdf",
    )
    .unwrap();

    assert!(document_ops::delete_document(&db.conn, document.id, false).is_err());
    assert!(document_ops::get_document(&db.conn, document.id).is_ok());
}

// --- Whole files: databases and backups (research.md §12) -------------------

mod whole_files {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use hoplodex_lib::services::secure_delete::{self, WipeControl, Wiped, testing};
    use tempfile::TempDir;

    const MIB: u64 = 1 << 20;

    /// A file of `len` non-zero bytes, and a second name for the same data
    /// that outlives the deletion, so the test can see what was left in it.
    fn file_with_witness(dir: &TempDir, len: u64) -> (PathBuf, PathBuf) {
        let path = dir.path().join("Backup.hoplodex");
        fs::write(&path, vec![0xA5u8; len as usize]).unwrap();
        let witness = dir.path().join("witness");
        fs::hard_link(&path, &witness).unwrap();
        (path, witness)
    }

    fn delete_with_progress(path: &Path) -> (std::io::Result<Wiped>, Vec<(u64, u64)>) {
        let mut reported = Vec::new();
        let mut progress = |done: u64, total: u64| reported.push((done, total));
        let result = secure_delete::secure_delete_whole_file(
            path,
            WipeControl { progress: Some(&mut progress), cancel: None },
        );
        (result, reported)
    }

    #[test]
    fn a_large_file_is_overwritten_in_one_mebibyte_chunks_with_progress() {
        let dir = TempDir::new().unwrap();
        let len = 2 * MIB + MIB / 2;
        let (path, witness) = file_with_witness(&dir, len);

        let (result, reported) = delete_with_progress(&path);

        assert!(matches!(result, Ok(Wiped::Deleted)), "{result:?}");
        assert!(!path.exists());
        assert_eq!(reported, vec![(MIB, len), (2 * MIB, len), (len, len)]);
        assert!(fs::read(&witness).unwrap().iter().all(|b| *b == 0), "the data was overwritten");
    }

    #[test]
    fn a_small_file_keeps_the_small_chunks() {
        let dir = TempDir::new().unwrap();
        let len = 200 * 1024;
        let (path, _witness) = file_with_witness(&dir, len);

        let (_, reported) = delete_with_progress(&path);

        let steps: Vec<u64> = reported.iter().map(|(done, _)| *done).collect();
        assert_eq!(steps, vec![64 * 1024, 128 * 1024, 192 * 1024, len]);
    }

    #[test]
    fn a_failed_discard_step_is_ignored() {
        let dir = TempDir::new().unwrap();
        let (path, witness) = file_with_witness(&dir, 3 * MIB);
        let _refused = testing::fail_discard();

        let (result, _) = delete_with_progress(&path);

        assert!(matches!(result, Ok(Wiped::Deleted)), "{result:?}");
        assert!(!path.exists());
        assert!(fs::read(&witness).unwrap().iter().all(|b| *b == 0));
    }

    #[test]
    fn a_stopped_overwrite_still_removes_the_file() {
        let dir = TempDir::new().unwrap();
        let (path, witness) = file_with_witness(&dir, 3 * MIB);
        let mut chunks = 0;
        let mut progress = |_: u64, _: u64| chunks += 1;
        // Asked before each chunk: go on once, then stop.
        let asked = std::cell::Cell::new(0);
        let stop_after_one = || {
            asked.set(asked.get() + 1);
            asked.get() > 1
        };

        let result = secure_delete::secure_delete_whole_file(
            &path,
            WipeControl { progress: Some(&mut progress), cancel: Some(&stop_after_one) },
        );

        assert!(matches!(result, Ok(Wiped::Stopped)), "{result:?}");
        assert!(!path.exists(), "nothing half overwritten is left to look like a backup");
        assert_eq!(chunks, 1);
        let left = fs::read(&witness).unwrap();
        assert!(left[..MIB as usize].iter().all(|b| *b == 0));
    }

    #[test]
    fn a_file_that_cannot_be_overwritten_is_left_alone_and_reported() {
        let dir = TempDir::new().unwrap();
        let (path, _witness) = file_with_witness(&dir, MIB);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();

        let (result, _) = delete_with_progress(&path);

        assert!(result.is_err());
        assert!(path.exists(), "unlinking it would leave its data unwiped");
    }
}
