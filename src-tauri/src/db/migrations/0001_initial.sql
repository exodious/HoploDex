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
-- number is never compared). Backstop for the check in the command layer,
-- which also trims surrounding whitespace.
CREATE UNIQUE INDEX idx_firearms_active_identity
    ON firearms (make COLLATE NOCASE, model COLLATE NOCASE, serial_number COLLATE NOCASE)
    WHERE status = 'active' AND serial_number IS NOT NULL;

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
