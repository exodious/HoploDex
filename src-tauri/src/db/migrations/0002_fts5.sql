-- FTS5 external-content index over firearms, per data-model.md's
-- "Virtual table: firearms_fts" section (FR-013, US2 Scenarios 3-4).
-- specs/004-cartridges-action-types FR-008/FR-020 adds the cartridge and the
-- action's name, looked up the way the type's name is.
-- specs/005-regulated-item-types FR-017 adds the classification's name and the
-- registration form and "registered to" text (not the approved date).
--
-- The trigram tokenizer matches any run of three or more characters inside a
-- value, so "365" finds the model "P365 XL" and "1911" finds serial "CO1911";
-- the default word tokenizer only matched from the start of a word. A search
-- of one or two characters can't use it (see `list_firearms`).

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
    cartridge,
    action_type_name,
    registered_as,
    registration_form,
    registered_to,
    content = 'firearms',
    content_rowid = 'id',
    tokenize = 'trigram remove_diacritics 1'
);

-- specs/002-firearm-identification FR-012: origin is indexed as its display
-- label so "imported"/"re-imported"/"domestic" search as the user reads them;
-- country_of_manufacture is indexed as "United States" for a re-imported
-- firearm, since that is what is displayed and searched (never stored).
CREATE TRIGGER firearms_fts_after_insert AFTER INSERT ON firearms
BEGIN
    INSERT INTO firearms_fts (
        rowid, make, model, nickname, serial_number, caliber, notes, accessories, finish, firearm_type_name,
        origin, year_of_manufacture, country_of_manufacture, importer_name, original_make, original_model, original_serial_number,
        cartridge, action_type_name, registered_as, registration_form, registered_to
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
        new.original_serial_number,
        new.cartridge,
        (SELECT name FROM action_types WHERE id = new.action_type_id),
        (SELECT name FROM registration_classes WHERE id = new.registration_class_id),
        new.registration_form,
        new.registered_to
    );
END;

CREATE TRIGGER firearms_fts_after_delete AFTER DELETE ON firearms
BEGIN
    INSERT INTO firearms_fts (
        firearms_fts, rowid, make, model, nickname, serial_number, caliber, notes, accessories, finish, firearm_type_name,
        origin, year_of_manufacture, country_of_manufacture, importer_name, original_make, original_model, original_serial_number,
        cartridge, action_type_name, registered_as, registration_form, registered_to
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
        old.original_serial_number,
        old.cartridge,
        (SELECT name FROM action_types WHERE id = old.action_type_id),
        (SELECT name FROM registration_classes WHERE id = old.registration_class_id),
        old.registration_form,
        old.registered_to
    );
END;

CREATE TRIGGER firearms_fts_after_update AFTER UPDATE ON firearms
BEGIN
    INSERT INTO firearms_fts (
        firearms_fts, rowid, make, model, nickname, serial_number, caliber, notes, accessories, finish, firearm_type_name,
        origin, year_of_manufacture, country_of_manufacture, importer_name, original_make, original_model, original_serial_number,
        cartridge, action_type_name, registered_as, registration_form, registered_to
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
        old.original_serial_number,
        old.cartridge,
        (SELECT name FROM action_types WHERE id = old.action_type_id),
        (SELECT name FROM registration_classes WHERE id = old.registration_class_id),
        old.registration_form,
        old.registered_to
    );
    INSERT INTO firearms_fts (
        rowid, make, model, nickname, serial_number, caliber, notes, accessories, finish, firearm_type_name,
        origin, year_of_manufacture, country_of_manufacture, importer_name, original_make, original_model, original_serial_number,
        cartridge, action_type_name, registered_as, registration_form, registered_to
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
        new.original_serial_number,
        new.cartridge,
        (SELECT name FROM action_types WHERE id = new.action_type_id),
        (SELECT name FROM registration_classes WHERE id = new.registration_class_id),
        new.registration_form,
        new.registered_to
    );
END;

-- specs/006-accessory-links (data-model.md's "Virtual table: accessories_fts",
-- research.md §12): the same index over accessories, with the kind's name
-- looked up the way a firearm's type name is. A search of one or two
-- characters uses LIKE over the same values.
CREATE VIRTUAL TABLE accessories_fts USING fts5(
    kind_name,
    make,
    model,
    serial_number,
    caliber,
    cartridge,
    acquisition_source,
    notes,
    content = 'accessories',
    content_rowid = 'id',
    tokenize = 'trigram remove_diacritics 1'
);

CREATE TRIGGER accessories_fts_after_insert AFTER INSERT ON accessories
BEGIN
    INSERT INTO accessories_fts (
        rowid, kind_name, make, model, serial_number, caliber, cartridge, acquisition_source, notes
    )
    VALUES (
        new.id,
        (SELECT name FROM accessory_kinds WHERE id = new.accessory_kind_id),
        new.make,
        new.model,
        new.serial_number,
        new.caliber,
        new.cartridge,
        new.acquisition_source,
        new.notes
    );
END;

CREATE TRIGGER accessories_fts_after_delete AFTER DELETE ON accessories
BEGIN
    INSERT INTO accessories_fts (
        accessories_fts, rowid, kind_name, make, model, serial_number, caliber, cartridge, acquisition_source, notes
    )
    VALUES (
        'delete',
        old.id,
        (SELECT name FROM accessory_kinds WHERE id = old.accessory_kind_id),
        old.make,
        old.model,
        old.serial_number,
        old.caliber,
        old.cartridge,
        old.acquisition_source,
        old.notes
    );
END;

CREATE TRIGGER accessories_fts_after_update AFTER UPDATE ON accessories
BEGIN
    INSERT INTO accessories_fts (
        accessories_fts, rowid, kind_name, make, model, serial_number, caliber, cartridge, acquisition_source, notes
    )
    VALUES (
        'delete',
        old.id,
        (SELECT name FROM accessory_kinds WHERE id = old.accessory_kind_id),
        old.make,
        old.model,
        old.serial_number,
        old.caliber,
        old.cartridge,
        old.acquisition_source,
        old.notes
    );
    INSERT INTO accessories_fts (
        rowid, kind_name, make, model, serial_number, caliber, cartridge, acquisition_source, notes
    )
    VALUES (
        new.id,
        (SELECT name FROM accessory_kinds WHERE id = new.accessory_kind_id),
        new.make,
        new.model,
        new.serial_number,
        new.caliber,
        new.cartridge,
        new.acquisition_source,
        new.notes
    );
END;
