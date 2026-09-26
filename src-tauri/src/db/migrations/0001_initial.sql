-- Initial schema per specs/001-firearms-inventory/data-model.md
--
-- Every price, value, coverage amount and limit is a whole number of U.S.
-- dollars, never cents (FR-037).

CREATE TABLE firearm_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    generic_thumbnail_key TEXT NOT NULL
);

CREATE TABLE insurance_policies (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    policy_number TEXT NOT NULL,
    insurance_company TEXT NOT NULL,
    company_contact TEXT,
    agent_name TEXT,
    agent_contact TEXT,
    -- FR-027: optional free-form notes; blank input is stored as null.
    notes TEXT,
    -- FR-027/FR-036: set => a blanket policy, whose limit is shared by every
    -- firearm not individually scheduled while the policy is in force.
    blanket_coverage_limit INTEGER CHECK (blanket_coverage_limit IS NULL OR blanket_coverage_limit >= 0),
    effective_start_date TEXT NOT NULL,
    effective_end_date TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (effective_end_date > effective_start_date)
);

-- References photos(id) below; SQLite resolves FK targets lazily so this
-- forward reference (created before the photos table) is valid.
CREATE TABLE firearms (
    id INTEGER PRIMARY KEY,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    serial_number TEXT,
    nickname TEXT,
    no_serial_attested INTEGER NOT NULL DEFAULT 0 CHECK (no_serial_attested IN (0, 1)),
    caliber TEXT NOT NULL,
    firearm_type_id INTEGER NOT NULL REFERENCES firearm_types (id),
    notes TEXT,
    accessories TEXT,
    -- FR-039: optional physical details. Lengths are stored in hundredths of
    -- an inch, weight in tenths of an ounce, so they stay exact integers.
    barrel_length_hundredths INTEGER CHECK (barrel_length_hundredths IS NULL OR barrel_length_hundredths > 0),
    overall_length_hundredths INTEGER CHECK (overall_length_hundredths IS NULL OR overall_length_hundredths > 0),
    weight_tenths_oz INTEGER CHECK (weight_tenths_oz IS NULL OR weight_tenths_oz > 0),
    capacity INTEGER CHECK (capacity IS NULL OR capacity >= 1),
    finish TEXT,
    condition TEXT
        CHECK (condition IS NULL OR condition IN ('new_in_box', 'like_new', 'excellent', 'good', 'fair', 'poor')),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disposed')),
    estimated_value INTEGER CHECK (estimated_value IS NULL OR estimated_value >= 0),
    acquisition_source TEXT,
    acquisition_date TEXT,
    acquisition_price INTEGER CHECK (acquisition_price IS NULL OR acquisition_price >= 0),
    disposition_type TEXT
        CHECK (
            disposition_type IS NULL
            OR disposition_type IN ('sold', 'traded', 'gifted', 'destroyed', 'lost_stolen')
        ),
    disposition_recipient TEXT,
    disposition_date TEXT,
    disposition_price INTEGER CHECK (disposition_price IS NULL OR disposition_price >= 0),
    thumbnail_photo_id INTEGER REFERENCES photos (id) ON DELETE SET NULL,
    insurance_policy_id INTEGER REFERENCES insurance_policies (id) ON DELETE RESTRICT,
    scheduled_coverage_amount INTEGER CHECK (scheduled_coverage_amount IS NULL OR scheduled_coverage_amount >= 0),
    -- specs/002-firearm-identification: how the firearm is identified and
    -- marked. `origin` NULL means not specified. See data-model.md's
    -- "Entity: Firearm (extended)".
    origin TEXT,
    year_of_manufacture INTEGER,
    country_of_manufacture TEXT,
    importer_name TEXT,
    original_make TEXT,
    original_model TEXT,
    original_serial_number TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    -- FR-029: a serial number, or the attestation that there is none; never both.
    CHECK ((serial_number IS NOT NULL) <> (no_serial_attested = 1)),
    -- FR-014/FR-036: scheduled under a policy with its own amount, or not
    -- scheduled at all. Blanket coverage is computed, never stored here.
    CHECK ((insurance_policy_id IS NULL) = (scheduled_coverage_amount IS NULL)),
    -- specs/002-firearm-identification FR-001..FR-004
    CHECK (origin IS NULL OR origin IN ('domestic', 'imported', 'reimported')),
    CHECK (year_of_manufacture IS NULL OR year_of_manufacture BETWEEN 1400 AND 9999),
    CHECK (country_of_manufacture IS NULL OR origin = 'imported'),
    CHECK (
        (importer_name IS NULL AND original_make IS NULL
         AND original_model IS NULL AND original_serial_number IS NULL)
        OR origin IN ('imported', 'reimported')
    )
);

CREATE INDEX idx_firearms_type ON firearms (firearm_type_id);
CREATE INDEX idx_firearms_caliber ON firearms (caliber);
CREATE INDEX idx_firearms_make ON firearms (make);
CREATE INDEX idx_firearms_status ON firearms (status);
CREATE INDEX idx_firearms_insurance_policy ON firearms (insurance_policy_id);

-- FR-031: a nickname is unique among active firearms (case-insensitively);
-- a disposed firearm releases it. Backstop for the check in the command
-- layer, which also trims surrounding whitespace.
CREATE UNIQUE INDEX idx_firearms_active_nickname
    ON firearms (nickname COLLATE NOCASE)
    WHERE status = 'active' AND nickname IS NOT NULL;

-- FR-032: make + model + serial number is unique among active firearms (a
-- disposed one may be reacquired as a new record; a record with no serial
-- number is never compared). specs/002-firearm-identification FR-007/FR-008
-- adds a year-of-manufacture exception a UNIQUE index cannot express (a pair
-- is allowed only when both years exist and differ, and SQLite treats NULLs
-- as distinct, which would wrongly allow two null-year records). So this is
-- now a plain lookup index — the triggers below are the real backstop — kept
-- for the identity queries in the command layer, which also trim whitespace.
CREATE INDEX idx_firearms_active_identity
    ON firearms (make COLLATE NOCASE, model COLLATE NOCASE, serial_number COLLATE NOCASE)
    WHERE status = 'active' AND serial_number IS NOT NULL;

-- specs/002-firearm-identification FR-007/FR-008 (data-model.md's "Indexes
-- and triggers"; amends 001's unique index above): the exact backstop for
-- the identity rule, since a unique index can no longer express the year
-- exception. Expected never to fire in normal use because the command layer
-- (`find_identity_clash`) checks first; reaching it means a bug bypassed
-- that layer (raw INSERT/UPDATE, e.g.), so `from_db` maps the raised ABORT
-- to INTERNAL_ERROR.
-- No `id <> NEW.id` here (unlike the UPDATE trigger below): a fresh INSERT's
-- row is new to the table and `NEW.id` may still be NULL (rowid not yet
-- assigned) at BEFORE INSERT time, when a NULL-vs-existing-id comparison
-- would silently exclude every row from the match.
CREATE TRIGGER firearms_active_identity_insert BEFORE INSERT ON firearms
WHEN NEW.status = 'active' AND NEW.serial_number IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'active firearm with the same make, model and serial number exists')
    WHERE EXISTS (
        SELECT 1 FROM firearms
        WHERE status = 'active'
          AND make COLLATE NOCASE = NEW.make
          AND model COLLATE NOCASE = NEW.model
          AND serial_number COLLATE NOCASE = NEW.serial_number
          AND NOT (
              NEW.year_of_manufacture IS NOT NULL AND year_of_manufacture IS NOT NULL
              AND year_of_manufacture <> NEW.year_of_manufacture
          )
    );
END;

CREATE TRIGGER firearms_active_identity_update BEFORE UPDATE ON firearms
WHEN NEW.status = 'active' AND NEW.serial_number IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'active firearm with the same make, model and serial number exists')
    WHERE EXISTS (
        SELECT 1 FROM firearms
        WHERE id <> NEW.id
          AND status = 'active'
          AND make COLLATE NOCASE = NEW.make
          AND model COLLATE NOCASE = NEW.model
          AND serial_number COLLATE NOCASE = NEW.serial_number
          AND NOT (
              NEW.year_of_manufacture IS NOT NULL AND year_of_manufacture IS NOT NULL
              AND year_of_manufacture <> NEW.year_of_manufacture
          )
    );
END;

-- specs/002-firearm-identification FR-009 (data-model.md's "Indexes and
-- triggers"): serves the original-marks warning lookup. Non-unique: the
-- warning never blocks.
CREATE INDEX idx_firearms_original_serial
    ON firearms (original_serial_number COLLATE NOCASE)
    WHERE status = 'active' AND original_serial_number IS NOT NULL;

CREATE TABLE photos (
    id INTEGER PRIMARY KEY,
    firearm_id INTEGER NOT NULL REFERENCES firearms (id) ON DELETE CASCADE,
    original_bytes BLOB NOT NULL,
    original_filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    thumbnail_bytes BLOB NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_photos_firearm ON photos (firearm_id);

CREATE TABLE document_attachments (
    id INTEGER PRIMARY KEY,
    firearm_id INTEGER NOT NULL REFERENCES firearms (id) ON DELETE CASCADE,
    file_bytes BLOB NOT NULL,
    original_filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_document_attachments_firearm ON document_attachments (firearm_id);

-- FR-033: past dispositions of a firearm that was restored to active, kept
-- only when the user chose to keep them. Deleted with the firearm.
CREATE TABLE disposition_history (
    id INTEGER PRIMARY KEY,
    firearm_id INTEGER NOT NULL REFERENCES firearms (id) ON DELETE CASCADE,
    disposition_type TEXT NOT NULL
        CHECK (disposition_type IN ('sold', 'traded', 'gifted', 'destroyed', 'lost_stolen')),
    disposition_recipient TEXT NOT NULL,
    disposition_date TEXT NOT NULL,
    disposition_price INTEGER CHECK (disposition_price IS NULL OR disposition_price >= 0),
    reversed_at TEXT NOT NULL
);

CREATE INDEX idx_disposition_history_firearm ON disposition_history (firearm_id);

-- specs/003-database-protection-management (data-model.md "Inside the
-- database"). The backup and lock settings travel with the database and are
-- collection data: changing them makes a backup due (FR-024, FR-025,
-- FR-034, FR-038). Exactly one row, created with the database.
CREATE TABLE collection_settings (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    backups_enabled INTEGER NOT NULL DEFAULT 1 CHECK (backups_enabled IN (0, 1)),
    backup_keep_count INTEGER NOT NULL DEFAULT 5 CHECK (backup_keep_count BETWEEN 1 AND 100),
    -- 'default' (a "HoploDex backups" folder next to the database, resolved
    -- on each computer) or an absolute path (research.md §7).
    backup_location TEXT NOT NULL DEFAULT 'default',
    -- Also decides whether sleep locks the database (FR-037).
    idle_lock_enabled INTEGER NOT NULL DEFAULT 1 CHECK (idle_lock_enabled IN (0, 1)),
    idle_lock_minutes INTEGER NOT NULL DEFAULT 10 CHECK (idle_lock_minutes BETWEEN 1 AND 240),
    lock_on_screen_lock INTEGER NOT NULL DEFAULT 0 CHECK (lock_on_screen_lock IN (0, 1))
);

-- Housekeeping the application writes on its own at create, open, close,
-- backup and restore. It never makes a backup due, so it has no
-- change-tracking triggers (research.md §5). Exactly one row.
CREATE TABLE app_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    -- 32 lowercase hex digits, random at creation: names backups and the
    -- saved-passphrase keyring entry (research.md §7, §10).
    database_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    -- The open marker (FR-032, research.md §6): which machine has the
    -- database open, its name as shown to the user (at most 255
    -- characters), and since when. Set or cleared as a whole.
    open_machine_id TEXT,
    open_machine_name TEXT,
    open_since TEXT,
    -- The backup record (FR-025): collection changes not yet in a backup,
    -- and when the latest backup was made (UTC).
    changes_waiting INTEGER NOT NULL DEFAULT 0 CHECK (changes_waiting IN (0, 1)),
    last_backup_at TEXT,
    -- FR-008: the disk-encryption note was dismissed.
    disk_encryption_note_dismissed INTEGER NOT NULL DEFAULT 0 CHECK (disk_encryption_note_dismissed IN (0, 1)),
    CHECK (
        (open_machine_id IS NULL) = (open_machine_name IS NULL)
        AND (open_machine_id IS NULL) = (open_since IS NULL)
    )
);

-- A form's unsaved input, kept when the database was locked mid-edit
-- (FR-039, research.md §16). At most one row, since one form is open at a
-- time. Housekeeping: never in a backup, and no change-tracking triggers.
CREATE TABLE pending_changes (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    kind TEXT NOT NULL CHECK (kind IN ('firearm', 'policy')),
    mode TEXT NOT NULL CHECK (mode IN ('add', 'edit', 'dispose', 'restore', 'coverage')),
    -- The record being edited; NULL only when adding. Not a foreign key: the
    -- record may have been deleted on another computer, and then the draft
    -- can only be discarded.
    target_id INTEGER,
    -- e.g. "Glock 19 — edit"; at most 200 characters.
    label TEXT NOT NULL,
    form_version INTEGER NOT NULL,
    values_json TEXT NOT NULL CHECK (length(values_json) <= 1048576),
    saved_at TEXT NOT NULL,
    CHECK (mode <> 'coverage' OR kind = 'firearm'),
    CHECK (target_id IS NOT NULL OR mode = 'add')
);

-- FR-025, research.md §5: every change to collection data records that a
-- backup is due, in the same transaction as the change, so a crash cannot
-- lose the fact. `backup_due_tracking_test.rs` fails if a new table has
-- neither these triggers nor a place on its housekeeping list.
CREATE TRIGGER firearms_marks_backup_due_after_insert AFTER INSERT ON firearms
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER firearms_marks_backup_due_after_update AFTER UPDATE ON firearms
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER firearms_marks_backup_due_after_delete AFTER DELETE ON firearms
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER photos_marks_backup_due_after_insert AFTER INSERT ON photos
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER photos_marks_backup_due_after_update AFTER UPDATE ON photos
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER photos_marks_backup_due_after_delete AFTER DELETE ON photos
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER document_attachments_marks_backup_due_after_insert AFTER INSERT ON document_attachments
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER document_attachments_marks_backup_due_after_update AFTER UPDATE ON document_attachments
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER document_attachments_marks_backup_due_after_delete AFTER DELETE ON document_attachments
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER disposition_history_marks_backup_due_after_insert AFTER INSERT ON disposition_history
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER disposition_history_marks_backup_due_after_update AFTER UPDATE ON disposition_history
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER disposition_history_marks_backup_due_after_delete AFTER DELETE ON disposition_history
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER insurance_policies_marks_backup_due_after_insert AFTER INSERT ON insurance_policies
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER insurance_policies_marks_backup_due_after_update AFTER UPDATE ON insurance_policies
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER insurance_policies_marks_backup_due_after_delete AFTER DELETE ON insurance_policies
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER firearm_types_marks_backup_due_after_insert AFTER INSERT ON firearm_types
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER firearm_types_marks_backup_due_after_update AFTER UPDATE ON firearm_types
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER firearm_types_marks_backup_due_after_delete AFTER DELETE ON firearm_types
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER collection_settings_marks_backup_due_after_insert AFTER INSERT ON collection_settings
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER collection_settings_marks_backup_due_after_update AFTER UPDATE ON collection_settings
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER collection_settings_marks_backup_due_after_delete AFTER DELETE ON collection_settings
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;
