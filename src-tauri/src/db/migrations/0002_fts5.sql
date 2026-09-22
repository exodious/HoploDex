-- FTS5 external-content index over firearms, per data-model.md's
-- "Virtual table: firearms_fts" section (FR-013, US2 Scenarios 3-4).

CREATE VIRTUAL TABLE firearms_fts USING fts5(
    make,
    model,
    nickname,
    serial_number,
    caliber,
    notes,
    accessories,
    finish,
    firearm_type_name,
    origin,
    year_of_manufacture,
    country_of_manufacture,
    importer_name,
    original_make,
    original_model,
    original_serial_number,
    content = 'firearms',
    content_rowid = 'id'
);

-- specs/002-firearm-identification FR-012: origin is indexed as its display
-- label so "imported"/"re-imported"/"domestic" search as the user reads them;
-- country_of_manufacture is indexed as "United States" for a re-imported
-- firearm, since that is what is displayed and searched (never stored).
CREATE TRIGGER firearms_fts_after_insert AFTER INSERT ON firearms
BEGIN
    INSERT INTO firearms_fts (
        rowid, make, model, nickname, serial_number, caliber, notes, accessories, finish, firearm_type_name,
        origin, year_of_manufacture, country_of_manufacture, importer_name, original_make, original_model, original_serial_number
    )
    VALUES (
        new.id,
        new.make,
        new.model,
        new.nickname,
        new.serial_number,
        new.caliber,
        new.notes,
        new.accessories,
        new.finish,
        (SELECT name FROM firearm_types WHERE id = new.firearm_type_id),
        CASE new.origin WHEN 'domestic' THEN 'Domestic' WHEN 'imported' THEN 'Imported' WHEN 'reimported' THEN 'Re-imported' END,
        new.year_of_manufacture,
        CASE WHEN new.origin = 'reimported' THEN 'United States' ELSE new.country_of_manufacture END,
        new.importer_name,
        new.original_make,
        new.original_model,
        new.original_serial_number
    );
END;

CREATE TRIGGER firearms_fts_after_delete AFTER DELETE ON firearms
BEGIN
    INSERT INTO firearms_fts (
        firearms_fts, rowid, make, model, nickname, serial_number, caliber, notes, accessories, finish, firearm_type_name,
        origin, year_of_manufacture, country_of_manufacture, importer_name, original_make, original_model, original_serial_number
    )
    VALUES (
        'delete',
        old.id,
        old.make,
        old.model,
        old.nickname,
        old.serial_number,
        old.caliber,
        old.notes,
        old.accessories,
        old.finish,
        (SELECT name FROM firearm_types WHERE id = old.firearm_type_id),
        CASE old.origin WHEN 'domestic' THEN 'Domestic' WHEN 'imported' THEN 'Imported' WHEN 'reimported' THEN 'Re-imported' END,
        old.year_of_manufacture,
        CASE WHEN old.origin = 'reimported' THEN 'United States' ELSE old.country_of_manufacture END,
        old.importer_name,
        old.original_make,
        old.original_model,
        old.original_serial_number
    );
END;

CREATE TRIGGER firearms_fts_after_update AFTER UPDATE ON firearms
BEGIN
    INSERT INTO firearms_fts (
        firearms_fts, rowid, make, model, nickname, serial_number, caliber, notes, accessories, finish, firearm_type_name,
        origin, year_of_manufacture, country_of_manufacture, importer_name, original_make, original_model, original_serial_number
    )
    VALUES (
        'delete',
        old.id,
        old.make,
        old.model,
        old.nickname,
        old.serial_number,
        old.caliber,
        old.notes,
        old.accessories,
        old.finish,
        (SELECT name FROM firearm_types WHERE id = old.firearm_type_id),
        CASE old.origin WHEN 'domestic' THEN 'Domestic' WHEN 'imported' THEN 'Imported' WHEN 'reimported' THEN 'Re-imported' END,
        old.year_of_manufacture,
        CASE WHEN old.origin = 'reimported' THEN 'United States' ELSE old.country_of_manufacture END,
        old.importer_name,
        old.original_make,
        old.original_model,
        old.original_serial_number
    );
    INSERT INTO firearms_fts (
        rowid, make, model, nickname, serial_number, caliber, notes, accessories, finish, firearm_type_name,
        origin, year_of_manufacture, country_of_manufacture, importer_name, original_make, original_model, original_serial_number
    )
    VALUES (
        new.id,
        new.make,
        new.model,
        new.nickname,
        new.serial_number,
        new.caliber,
        new.notes,
        new.accessories,
        new.finish,
        (SELECT name FROM firearm_types WHERE id = new.firearm_type_id),
        CASE new.origin WHEN 'domestic' THEN 'Domestic' WHEN 'imported' THEN 'Imported' WHEN 'reimported' THEN 'Re-imported' END,
        new.year_of_manufacture,
        CASE WHEN new.origin = 'reimported' THEN 'United States' ELSE new.country_of_manufacture END,
        new.importer_name,
        new.original_make,
        new.original_model,
        new.original_serial_number
    );
END;
