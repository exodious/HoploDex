-- Initial schema per specs/001-firearms-inventory/data-model.md
--
-- Every price, value, coverage amount and limit is a whole number of U.S.
-- dollars, never cents (FR-037).

CREATE TABLE firearm_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    generic_thumbnail_key TEXT NOT NULL,
    -- The list's order: Other, the catch-all, comes last.
    sort_order INTEGER NOT NULL UNIQUE,
    -- specs/005-regulated-item-types FR-003: 0 = the field doesn't apply to
    -- this type: the form doesn't offer it and no firearm of the type may
    -- hold a value.
    action_type_applies INTEGER NOT NULL DEFAULT 1 CHECK (action_type_applies IN (0, 1)),
    barrel_length_applies INTEGER NOT NULL DEFAULT 1 CHECK (barrel_length_applies IN (0, 1)),
    capacity_applies INTEGER NOT NULL DEFAULT 1 CHECK (capacity_applies IN (0, 1)),
    -- specs/005-regulated-item-types FR-002 (research.md §15): 0 = the
    -- caliber is never worked out from the cartridge, on the form or on
    -- import. A Suppressor's caliber is its bore, its cartridge its rating.
    caliber_from_cartridge INTEGER NOT NULL DEFAULT 1 CHECK (caliber_from_cartridge IN (0, 1))
);

-- specs/004-cartridges-action-types FR-017/FR-018 (data-model.md's "Entity:
-- Action Type"): the fixed list of how a firearm operates, seeded in 0003
-- with fixed ids and never changed at run time.
CREATE TABLE action_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL UNIQUE
);

-- specs/004-cartridges-action-types FR-017/FR-018: which actions apply to
-- which firearm type. A type with no rows here allows every action.
CREATE TABLE firearm_type_actions (
    firearm_type_id INTEGER NOT NULL REFERENCES firearm_types (id) ON DELETE CASCADE,
    action_type_id INTEGER NOT NULL REFERENCES action_types (id),
    PRIMARY KEY (firearm_type_id, action_type_id)
) WITHOUT ROWID;

-- specs/005-regulated-item-types FR-007 (data-model.md's "Entity:
-- Registration Classification"): the fixed list of what a firearm can be
-- registered as, seeded in 0003 with fixed ids and never changed at run time.
CREATE TABLE registration_classes (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL UNIQUE,
    -- FR-007: 0 = no longer offered for new choices. The row is never
    -- deleted or renamed while a record can hold it.
    offered INTEGER NOT NULL DEFAULT 1 CHECK (offered IN (0, 1))
);

-- specs/006-accessory-links FR-002 (data-model.md's "Entity: Accessory
-- Kind"): the fixed list of what an accessory can be, seeded in 0003 with
-- fixed ids and never changed at run time.
CREATE TABLE accessory_kinds (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    generic_thumbnail_key TEXT NOT NULL,
    sort_order INTEGER NOT NULL UNIQUE,
    -- FR-002: 0 = no longer offered for new choices. The row is never
    -- deleted or renamed while a record can hold it.
    offered INTEGER NOT NULL DEFAULT 1 CHECK (offered IN (0, 1))
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
    -- specs/006-accessory-links FR-019/FR-022 (data-model.md "Entity: Record
    -- Identifier"): a random version 4 UUID, lowercase and hyphenated, set at
    -- creation and never changed. No IPC output carries it.
    uid TEXT NOT NULL UNIQUE CHECK (
        length(uid) = 36
        AND uid GLOB '????????-????-4???-????-????????????'
        AND substr(uid, 20, 1) IN ('8', '9', 'a', 'b')
        AND NOT (replace(uid, '-', '') GLOB '*[^0-9a-f]*')
    ),
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    serial_number TEXT,
    nickname TEXT,
    no_serial_attested INTEGER NOT NULL DEFAULT 0 CHECK (no_serial_attested IN (0, 1)),
    caliber TEXT NOT NULL,
    -- specs/004-cartridges-action-types FR-001: the exact round, free text
    -- (NULL = none). Stored on the record, never looked up in the catalog
    -- (FR-004). The 100-character cap applies on entry only (FR-015), so it
    -- has no CHECK: an existing longer value stays valid.
    cartridge TEXT,
    firearm_type_id INTEGER NOT NULL REFERENCES firearm_types (id),
    -- specs/004-cartridges-action-types FR-017: NULL = not specified. Must
    -- be allowed for the type (the triggers below).
    action_type_id INTEGER REFERENCES action_types (id),
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
    -- specs/005-regulated-item-types FR-007/FR-009: what the item is
    -- registered as (NULL = none) and the optional details of that
    -- registration. The text fields carry no length CHECK: 004's entry rules
    -- apply on entry only.
    registration_class_id INTEGER REFERENCES registration_classes (id),
    registration_form TEXT,
    registration_approved TEXT,
    registered_to TEXT,
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
    ),
    -- specs/005-regulated-item-types FR-009: no details without a classification.
    CHECK (
        registration_class_id IS NOT NULL
        OR (registration_form IS NULL AND registration_approved IS NULL AND registered_to IS NULL)
    )
);

CREATE INDEX idx_firearms_type ON firearms (firearm_type_id);
CREATE INDEX idx_firearms_caliber ON firearms (caliber);
-- specs/004-cartridges-action-types research.md §5: covers suggest_entries'
-- GROUP BY cartridge.
CREATE INDEX idx_firearms_cartridge ON firearms (cartridge);
-- specs/005-regulated-item-types research.md §7: cover suggest_entries'
-- GROUP BY for the two registration text fields.
CREATE INDEX idx_firearms_registered_to ON firearms (registered_to) WHERE registered_to IS NOT NULL;
CREATE INDEX idx_firearms_registration_form ON firearms (registration_form) WHERE registration_form IS NOT NULL;
CREATE INDEX idx_firearms_make ON firearms (make);
-- specs/004-cartridges-action-types research.md §5 (T043): covers the model
-- suggestions' GROUP BY make, model, which performance_test.rs holds to 50ms
-- at 10,000 distinct models.
CREATE INDEX idx_firearms_make_model ON firearms (make, model);
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

-- specs/004-cartridges-action-types FR-017/SC-008 (data-model.md's "Rule:
-- action allowed for type"): the backstop for `check_action_allowed` in the
-- command layer. An action is allowed when the type maps no actions, or maps
-- this one. Reaching it means a bug bypassed that layer, so `from_db` maps
-- the raised ABORT to INTERNAL_ERROR.
CREATE TRIGGER firearms_action_allowed_insert BEFORE INSERT ON firearms
WHEN NEW.action_type_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'action type not allowed for this firearm type')
    WHERE EXISTS (SELECT 1 FROM firearm_type_actions WHERE firearm_type_id = NEW.firearm_type_id)
      AND NOT EXISTS (
          SELECT 1 FROM firearm_type_actions
          WHERE firearm_type_id = NEW.firearm_type_id AND action_type_id = NEW.action_type_id
      );
END;

CREATE TRIGGER firearms_action_allowed_update BEFORE UPDATE OF action_type_id, firearm_type_id ON firearms
WHEN NEW.action_type_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'action type not allowed for this firearm type')
    WHERE EXISTS (SELECT 1 FROM firearm_type_actions WHERE firearm_type_id = NEW.firearm_type_id)
      AND NOT EXISTS (
          SELECT 1 FROM firearm_type_actions
          WHERE firearm_type_id = NEW.firearm_type_id AND action_type_id = NEW.action_type_id
      );
END;

-- specs/005-regulated-item-types FR-003/SC-005: the backstop for
-- `check_fields_apply` in the command layer, which runs first. A firearm may
-- hold an action, a barrel length or a capacity only when its type's flag
-- says the field applies. Reaching this means a bug bypassed that layer, so
-- `from_db` maps the raised ABORT to INTERNAL_ERROR.
CREATE TRIGGER firearms_fields_apply_insert BEFORE INSERT ON firearms
WHEN NEW.action_type_id IS NOT NULL OR NEW.barrel_length_hundredths IS NOT NULL OR NEW.capacity IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a field that does not apply to this firearm type has a value')
    FROM firearm_types t
    WHERE t.id = NEW.firearm_type_id
      AND ((NEW.action_type_id IS NOT NULL AND t.action_type_applies = 0)
        OR (NEW.barrel_length_hundredths IS NOT NULL AND t.barrel_length_applies = 0)
        OR (NEW.capacity IS NOT NULL AND t.capacity_applies = 0));
END;

CREATE TRIGGER firearms_fields_apply_update BEFORE UPDATE OF firearm_type_id, action_type_id, barrel_length_hundredths, capacity ON firearms
WHEN NEW.action_type_id IS NOT NULL OR NEW.barrel_length_hundredths IS NOT NULL OR NEW.capacity IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a field that does not apply to this firearm type has a value')
    FROM firearm_types t
    WHERE t.id = NEW.firearm_type_id
      AND ((NEW.action_type_id IS NOT NULL AND t.action_type_applies = 0)
        OR (NEW.barrel_length_hundredths IS NOT NULL AND t.barrel_length_applies = 0)
        OR (NEW.capacity IS NOT NULL AND t.capacity_applies = 0));
END;

-- specs/002-firearm-identification FR-009 (data-model.md's "Indexes and
-- triggers"): serves the original-marks warning lookup. Non-unique: the
-- warning never blocks.
CREATE INDEX idx_firearms_original_serial
    ON firearms (original_serial_number COLLATE NOCASE)
    WHERE status = 'active' AND original_serial_number IS NOT NULL;

-- specs/006-accessory-links (data-model.md's "Entity: Accessory (new)"):
-- a part or add-on that is its own record. It has no nickname, no
-- serial-or-attestation CHECK, no identity index and no fields-apply trigger
-- (FR-004). References photos(id) below, like firearms does.
CREATE TABLE accessories (
    id INTEGER PRIMARY KEY,
    uid TEXT NOT NULL UNIQUE CHECK (
        length(uid) = 36
        AND uid GLOB '????????-????-4???-????-????????????'
        AND substr(uid, 20, 1) IN ('8', '9', 'a', 'b')
        AND NOT (replace(uid, '-', '') GLOB '*[^0-9a-f]*')
    ),
    accessory_kind_id INTEGER NOT NULL REFERENCES accessory_kinds (id),
    -- FR-001: all optional. 004's entry rules apply on entry only, so no
    -- length CHECK (an existing longer value stays valid).
    make TEXT,
    model TEXT,
    serial_number TEXT,
    caliber TEXT,
    cartridge TEXT,
    notes TEXT,
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
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK ((insurance_policy_id IS NULL) = (scheduled_coverage_amount IS NULL))
);

CREATE INDEX idx_accessories_kind ON accessories (accessory_kind_id);
CREATE INDEX idx_accessories_status ON accessories (status);
CREATE INDEX idx_accessories_insurance_policy ON accessories (insurance_policy_id);
-- research.md §16: suggest_entries' GROUP BY over both tables.
CREATE INDEX idx_accessories_make ON accessories (make) WHERE make IS NOT NULL;
CREATE INDEX idx_accessories_make_model ON accessories (make, model) WHERE model IS NOT NULL;
CREATE INDEX idx_accessories_caliber ON accessories (caliber) WHERE caliber IS NOT NULL;
CREATE INDEX idx_accessories_cartridge ON accessories (cartridge) WHERE cartridge IS NOT NULL;

-- FR-019: a record identifier never changes. Backstops, expected never to
-- fire: `from_db` maps the raised ABORT to INTERNAL_ERROR.
CREATE TRIGGER firearms_uid_fixed BEFORE UPDATE OF uid ON firearms
WHEN NEW.uid IS NOT OLD.uid
BEGIN SELECT RAISE(ABORT, 'a record identifier never changes'); END;

CREATE TRIGGER accessories_uid_fixed BEFORE UPDATE OF uid ON accessories
WHEN NEW.uid IS NOT OLD.uid
BEGIN SELECT RAISE(ABORT, 'a record identifier never changes'); END;

-- FR-022: one identifier names one record across both tables.
CREATE TRIGGER accessories_uid_distinct BEFORE INSERT ON accessories
BEGIN
    SELECT RAISE(ABORT, 'record identifier already used by a firearm')
    WHERE EXISTS (SELECT 1 FROM firearms WHERE uid = NEW.uid);
END;

CREATE TRIGGER firearms_uid_distinct BEFORE INSERT ON firearms
BEGIN
    SELECT RAISE(ABORT, 'record identifier already used by an accessory')
    WHERE EXISTS (SELECT 1 FROM accessories WHERE uid = NEW.uid);
END;

CREATE TABLE photos (
    id INTEGER PRIMARY KEY,
    -- specs/006-accessory-links: owned by a firearm or by an accessory,
    -- never both (data-model.md "Entity: Photo, Document Attachment,
    -- Disposition History").
    firearm_id INTEGER REFERENCES firearms (id) ON DELETE CASCADE,
    accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE,
    original_bytes BLOB NOT NULL,
    original_filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    thumbnail_bytes BLOB NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    CHECK ((firearm_id IS NULL) <> (accessory_id IS NULL))
);

CREATE INDEX idx_photos_firearm ON photos (firearm_id);
CREATE INDEX idx_photos_accessory ON photos (accessory_id) WHERE accessory_id IS NOT NULL;

CREATE TABLE document_attachments (
    id INTEGER PRIMARY KEY,
    firearm_id INTEGER REFERENCES firearms (id) ON DELETE CASCADE,
    accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE,
    file_bytes BLOB NOT NULL,
    original_filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK ((firearm_id IS NULL) <> (accessory_id IS NULL))
);

CREATE INDEX idx_document_attachments_firearm ON document_attachments (firearm_id);
CREATE INDEX idx_document_attachments_accessory ON document_attachments (accessory_id) WHERE accessory_id IS NOT NULL;

-- FR-033: past dispositions of a firearm that was restored to active, kept
-- only when the user chose to keep them. Deleted with the firearm.
CREATE TABLE disposition_history (
    id INTEGER PRIMARY KEY,
    firearm_id INTEGER REFERENCES firearms (id) ON DELETE CASCADE,
    accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE,
    disposition_type TEXT NOT NULL
        CHECK (disposition_type IN ('sold', 'traded', 'gifted', 'destroyed', 'lost_stolen')),
    disposition_recipient TEXT NOT NULL,
    disposition_date TEXT NOT NULL,
    disposition_price INTEGER CHECK (disposition_price IS NULL OR disposition_price >= 0),
    reversed_at TEXT NOT NULL,
    CHECK ((firearm_id IS NULL) <> (accessory_id IS NULL))
);

CREATE INDEX idx_disposition_history_firearm ON disposition_history (firearm_id);
CREATE INDEX idx_disposition_history_accessory ON disposition_history (accessory_id) WHERE accessory_id IS NOT NULL;

-- specs/006-accessory-links (data-model.md's "Entity: Mount (new)"): where an
-- item is mounted. An item has at most one host (UNIQUE item columns); the
-- command layer moves it by updating its row. Unmounting deletes the row, so
-- there is no history (secure_delete is on).
CREATE TABLE mounts (
    id INTEGER PRIMARY KEY,
    item_firearm_id   INTEGER UNIQUE REFERENCES firearms (id)    ON DELETE CASCADE,
    item_accessory_id INTEGER UNIQUE REFERENCES accessories (id) ON DELETE CASCADE,
    host_firearm_id   INTEGER REFERENCES firearms (id)    ON DELETE CASCADE,
    host_accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE,
    -- Exactly one item and one host.
    CHECK ((item_firearm_id IS NULL) <> (item_accessory_id IS NULL)),
    CHECK ((host_firearm_id IS NULL) <> (host_accessory_id IS NULL)),
    -- FR-010: never mounted on itself (one step; longer loops: the command layer).
    CHECK (item_firearm_id IS NULL OR item_firearm_id IS NOT host_firearm_id),
    CHECK (item_accessory_id IS NULL OR item_accessory_id IS NOT host_accessory_id)
);

CREATE INDEX idx_mounts_host_firearm ON mounts (host_firearm_id) WHERE host_firearm_id IS NOT NULL;
CREATE INDEX idx_mounts_host_accessory ON mounts (host_accessory_id) WHERE host_accessory_id IS NOT NULL;

-- FR-013 backstops, expected never to fire (the command layer checks first;
-- `from_db` maps the raised ABORT to INTERNAL_ERROR): a mount needs an active
-- item and an active host, and a record is never disposed while it is in a
-- mount (FR-014: the dispose commands delete its mounts first).
CREATE TRIGGER mounts_active_insert BEFORE INSERT ON mounts
BEGIN
    SELECT RAISE(ABORT, 'a mount needs an active item and an active host')
    WHERE COALESCE((SELECT status FROM firearms WHERE id = NEW.item_firearm_id),
                   (SELECT status FROM accessories WHERE id = NEW.item_accessory_id)) IS NOT 'active'
       OR COALESCE((SELECT status FROM firearms WHERE id = NEW.host_firearm_id),
                   (SELECT status FROM accessories WHERE id = NEW.host_accessory_id)) IS NOT 'active';
END;

CREATE TRIGGER mounts_active_update BEFORE UPDATE ON mounts
BEGIN
    SELECT RAISE(ABORT, 'a mount needs an active item and an active host')
    WHERE COALESCE((SELECT status FROM firearms WHERE id = NEW.item_firearm_id),
                   (SELECT status FROM accessories WHERE id = NEW.item_accessory_id)) IS NOT 'active'
       OR COALESCE((SELECT status FROM firearms WHERE id = NEW.host_firearm_id),
                   (SELECT status FROM accessories WHERE id = NEW.host_accessory_id)) IS NOT 'active';
END;

CREATE TRIGGER firearms_disposed_unmounted BEFORE UPDATE OF status ON firearms
WHEN NEW.status = 'disposed'
BEGIN
    SELECT RAISE(ABORT, 'a disposed record cannot be mounted or carry mounts')
    WHERE EXISTS (SELECT 1 FROM mounts WHERE item_firearm_id = NEW.id OR host_firearm_id = NEW.id);
END;

CREATE TRIGGER accessories_disposed_unmounted BEFORE UPDATE OF status ON accessories
WHEN NEW.status = 'disposed'
BEGIN
    SELECT RAISE(ABORT, 'a disposed record cannot be mounted or carry mounts')
    WHERE EXISTS (SELECT 1 FROM mounts WHERE item_accessory_id = NEW.id OR host_accessory_id = NEW.id);
END;

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
    -- The backup stamp (research.md §9): UTC ISO-8601; set only in backup copies.
    backup_made_at TEXT,
    -- The database name, for the "backup opened directly" note; set only in backup copies.
    backup_of_name TEXT,
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
    kind TEXT NOT NULL CHECK (kind IN ('firearm', 'policy', 'accessory')),
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
    CHECK (mode <> 'coverage' OR kind IN ('firearm', 'accessory')),
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

CREATE TRIGGER action_types_marks_backup_due_after_insert AFTER INSERT ON action_types
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER action_types_marks_backup_due_after_update AFTER UPDATE ON action_types
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER action_types_marks_backup_due_after_delete AFTER DELETE ON action_types
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER registration_classes_marks_backup_due_after_insert AFTER INSERT ON registration_classes
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER registration_classes_marks_backup_due_after_update AFTER UPDATE ON registration_classes
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER registration_classes_marks_backup_due_after_delete AFTER DELETE ON registration_classes
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER firearm_type_actions_marks_backup_due_after_insert AFTER INSERT ON firearm_type_actions
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER firearm_type_actions_marks_backup_due_after_update AFTER UPDATE ON firearm_type_actions
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER firearm_type_actions_marks_backup_due_after_delete AFTER DELETE ON firearm_type_actions
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER collection_settings_marks_backup_due_after_insert AFTER INSERT ON collection_settings
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER collection_settings_marks_backup_due_after_update AFTER UPDATE ON collection_settings
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER collection_settings_marks_backup_due_after_delete AFTER DELETE ON collection_settings
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER accessory_kinds_marks_backup_due_after_insert AFTER INSERT ON accessory_kinds
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER accessory_kinds_marks_backup_due_after_update AFTER UPDATE ON accessory_kinds
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER accessory_kinds_marks_backup_due_after_delete AFTER DELETE ON accessory_kinds
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER accessories_marks_backup_due_after_insert AFTER INSERT ON accessories
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER accessories_marks_backup_due_after_update AFTER UPDATE ON accessories
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER accessories_marks_backup_due_after_delete AFTER DELETE ON accessories
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

-- specs/006-accessory-links: a mount is collection data.
CREATE TRIGGER mounts_marks_backup_due_after_insert AFTER INSERT ON mounts
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER mounts_marks_backup_due_after_update AFTER UPDATE ON mounts
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;

CREATE TRIGGER mounts_marks_backup_due_after_delete AFTER DELETE ON mounts
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;
