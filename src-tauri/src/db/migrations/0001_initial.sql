-- Initial schema per specs/001-firearms-inventory/data-model.md

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
    blanket_coverage_limit INTEGER NOT NULL,
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
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disposed')),
    estimated_value INTEGER,
    acquisition_source TEXT,
    acquisition_date TEXT,
    acquisition_price INTEGER,
    disposition_type TEXT
        CHECK (
            disposition_type IS NULL
            OR disposition_type IN ('sold', 'traded', 'gifted', 'destroyed', 'lost_stolen')
        ),
    disposition_recipient TEXT,
    disposition_date TEXT,
    disposition_price INTEGER,
    thumbnail_photo_id INTEGER REFERENCES photos (id) ON DELETE SET NULL,
    insurance_policy_id INTEGER REFERENCES insurance_policies (id) ON DELETE RESTRICT,
    coverage_kind TEXT
        CHECK (
            coverage_kind IS NULL
            OR coverage_kind IN ('individually_scheduled', 'blanket')
        ),
    scheduled_coverage_amount INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (serial_number IS NOT NULL OR no_serial_attested = 1)
);

CREATE INDEX idx_firearms_type ON firearms (firearm_type_id);
CREATE INDEX idx_firearms_caliber ON firearms (caliber);
CREATE INDEX idx_firearms_make ON firearms (make);
CREATE INDEX idx_firearms_status ON firearms (status);
CREATE INDEX idx_firearms_insurance_policy ON firearms (insurance_policy_id);

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
