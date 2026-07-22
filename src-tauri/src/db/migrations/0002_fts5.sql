-- FTS5 external-content index over firearms, per data-model.md's
-- "Virtual table: firearms_fts" section (FR-013, US2 Scenarios 3-4).

CREATE VIRTUAL TABLE firearms_fts USING fts5(
    make,
    model,
    serial_number,
    caliber,
    notes,
    accessories,
    firearm_type_name,
    content = 'firearms',
    content_rowid = 'id'
);

CREATE TRIGGER firearms_fts_after_insert AFTER INSERT ON firearms
BEGIN
    INSERT INTO firearms_fts (
        rowid, make, model, serial_number, caliber, notes, accessories, firearm_type_name
    )
    VALUES (
        new.id,
        new.make,
        new.model,
        new.serial_number,
        new.caliber,
        new.notes,
        new.accessories,
        (SELECT name FROM firearm_types WHERE id = new.firearm_type_id)
    );
END;

CREATE TRIGGER firearms_fts_after_delete AFTER DELETE ON firearms
BEGIN
    INSERT INTO firearms_fts (
        firearms_fts, rowid, make, model, serial_number, caliber, notes, accessories, firearm_type_name
    )
    VALUES (
        'delete',
        old.id,
        old.make,
        old.model,
        old.serial_number,
        old.caliber,
        old.notes,
        old.accessories,
        (SELECT name FROM firearm_types WHERE id = old.firearm_type_id)
    );
END;

CREATE TRIGGER firearms_fts_after_update AFTER UPDATE ON firearms
BEGIN
    INSERT INTO firearms_fts (
        firearms_fts, rowid, make, model, serial_number, caliber, notes, accessories, firearm_type_name
    )
    VALUES (
        'delete',
        old.id,
        old.make,
        old.model,
        old.serial_number,
        old.caliber,
        old.notes,
        old.accessories,
        (SELECT name FROM firearm_types WHERE id = old.firearm_type_id)
    );
    INSERT INTO firearms_fts (
        rowid, make, model, serial_number, caliber, notes, accessories, firearm_type_name
    )
    VALUES (
        new.id,
        new.make,
        new.model,
        new.serial_number,
        new.caliber,
        new.notes,
        new.accessories,
        (SELECT name FROM firearm_types WHERE id = new.firearm_type_id)
    );
END;
