//! Confirms the FTS5 index (data-model.md's "Virtual table: firearms_fts")
//! actually matches both free-form notes and structured fields, per FR-013.

mod support;

use hoplodex_lib::commands::firearms::{ListFirearmsInput, ops};
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use support::TestDb;

fn base_input() -> FirearmInput {
    FirearmInput {
        make: "Colt".into(),
        model: "1911".into(),
        serial_number: Some("CO1911".into()),
        no_serial_attested: false,
        caliber: ".45 ACP".into(),
        firearm_type_id: 1,
        notes: Some("inherited from grandfather, minor pitting on barrel".into()),
        accessories: Some("leather holster; spare magazine".into()),
        barrel_length_hundredths: None,
        overall_length_hundredths: None,
        weight_tenths_oz: None,
        capacity: None,
        finish: None,
        condition: None,
        status: FirearmStatus::Active,
        estimated_value: None,
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        nickname: None,
        scheduled_coverage_amount: None,
        origin: None,
        year_of_manufacture: None,
        country_of_manufacture: None,
        importer_name: None,
        original_make: None,
        original_model: None,
        original_serial_number: None,
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        cartridge: None,
        action_type_id: None,
    }
}

fn search(conn: &rusqlite::Connection, query: &str) -> usize {
    let result = ops::list_firearms(
        conn,
        &ListFirearmsInput { query: Some(query.into()), ..Default::default() },
    )
    .unwrap();
    result.groups.iter().flat_map(|g| &g.firearms).count()
}

#[test]
fn matches_structured_fields() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input(), false).unwrap();

    assert_eq!(search(&db.conn, "Colt"), 1, "should match make");
    assert_eq!(search(&db.conn, "1911"), 1, "should match model");
    assert_eq!(search(&db.conn, "CO1911"), 1, "should match serial number");
    assert_eq!(search(&db.conn, "45 ACP"), 1, "should match caliber");
}

#[test]
fn matches_free_form_notes_and_accessories() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input(), false).unwrap();

    assert_eq!(search(&db.conn, "grandfather"), 1, "should match free-form notes");
    assert_eq!(search(&db.conn, "holster"), 1, "should match accessories");
}

#[test]
fn matches_the_joined_firearm_type_name() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input(), false).unwrap();

    assert_eq!(search(&db.conn, "Handgun"), 1, "should match firearm_type.name via the join");
}

#[test]
fn stays_in_sync_after_update_and_delete() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &base_input(), false).unwrap();

    let mut edited = base_input();
    edited.notes = Some("re-blued and refinished".into());
    ops::update_firearm(&db.conn, created.id, &edited, false).unwrap();

    assert_eq!(search(&db.conn, "grandfather"), 0, "stale note text must not still match");
    assert_eq!(search(&db.conn, "re-blued"), 1, "updated note text must match");

    ops::delete_firearm(&db.conn, created.id, true).unwrap();
    assert_eq!(search(&db.conn, "re-blued"), 0, "deleted firearm must not match");
}

#[test]
fn no_match_returns_an_empty_result() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input(), false).unwrap();

    assert_eq!(search(&db.conn, "nonexistentxyz"), 0);
}

/// Search runs as the user types, so a partially typed last word must
/// already match ("Rem" finds Remington, "CO19" finds serial CO1911) —
/// whole-word-only matching showed nothing until the word was complete.
#[test]
fn matches_a_partially_typed_last_word() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input(), false).unwrap();

    assert_eq!(search(&db.conn, "Col"), 1, "prefix of make");
    assert_eq!(search(&db.conn, "CO19"), 1, "prefix of serial number");
    assert_eq!(search(&db.conn, "minor pit"), 1, "phrase ending in a partial word");
    assert_eq!(search(&db.conn, "pitting minor"), 0, "earlier words still form a phrase");
    assert_eq!(search(&db.conn, "\""), 0, "a stray quote is not a syntax error");
}

/// Regression: typing "365" found nothing for a "P365 XL" because the word
/// tokenizer only matched from the start of a word. Any run inside a value
/// matches now, and one or two characters still work while typing.
#[test]
fn matches_text_from_the_middle_of_a_value() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &FirearmInput { make: "Sig Sauer".into(), model: "P365 XL".into(), ..base_input() },
        false,
    )
    .unwrap();

    assert_eq!(search(&db.conn, "365"), 1, "digits after a letter");
    assert_eq!(search(&db.conn, "P365"), 1, "the whole word");
    assert_eq!(search(&db.conn, "365 x"), 1, "a phrase starting mid-word");
    assert_eq!(search(&db.conn, "911"), 1, "the middle of the serial number");
    assert_eq!(search(&db.conn, "sig sa"), 1, "a phrase ending in a partial word");
    assert_eq!(search(&db.conn, "366"), 0, "no false match");
    assert_eq!(search(&db.conn, "XL"), 1, "two characters");
    assert_eq!(search(&db.conn, "x"), 1, "one character");
    assert_eq!(search(&db.conn, "%"), 0, "a LIKE wildcard is literal");
    assert_eq!(search(&db.conn, "_"), 0, "a LIKE wildcard is literal");
}

/// FR-039 / US1 Acceptance Scenario 17: a word in a firearm's finish is found.
#[test]
fn matches_a_word_in_the_finish_including_after_an_edit_and_a_delete() {
    let db = TestDb::new();
    let created = ops::create_firearm(
        &db.conn,
        &FirearmInput { finish: Some("Cerakote flat dark earth".into()), ..base_input() },
        false,
    )
    .unwrap();
    assert_eq!(search(&db.conn, "Cerakote"), 1, "should match finish");

    let edited = FirearmInput { finish: Some("Parkerized".into()), ..base_input() };
    ops::update_firearm(&db.conn, created.id, &edited, false).unwrap();
    assert_eq!(search(&db.conn, "Cerakote"), 0, "the old finish is no longer indexed");
    assert_eq!(search(&db.conn, "Parkerized"), 1, "the edited finish is indexed");

    ops::delete_firearm(&db.conn, created.id, true).unwrap();
    assert_eq!(search(&db.conn, "Parkerized"), 0, "a deleted firearm is not found");
}

// specs/002-firearm-identification US1-4 / FR-012: origin, year, importer
// name and country of manufacture are searchable, and origin searches as
// its display label.

#[test]
fn searching_imported_finds_both_imported_and_reimported_but_not_domestic_or_none() {
    use hoplodex_lib::models::firearm::Origin;

    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            serial_number: Some("ORI-1".into()),
            origin: Some(Origin::Imported),
            ..base_input()
        },
        false,
    )
    .unwrap();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            serial_number: Some("ORI-2".into()),
            origin: Some(Origin::Reimported),
            ..base_input()
        },
        false,
    )
    .unwrap();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            serial_number: Some("ORI-3".into()),
            origin: Some(Origin::Domestic),
            ..base_input()
        },
        false,
    )
    .unwrap();
    ops::create_firearm(
        &db.conn,
        &FirearmInput { serial_number: Some("ORI-4".into()), origin: None, ..base_input() },
        false,
    )
    .unwrap();

    assert_eq!(search(&db.conn, "imported"), 2, "finds imported and re-imported");
    assert_eq!(search(&db.conn, "re-imported"), 1, "finds only re-imported");
    assert_eq!(search(&db.conn, "domestic"), 1, "finds only domestic");
}

#[test]
fn searching_year_importer_and_country_finds_the_firearm() {
    use hoplodex_lib::models::firearm::Origin;

    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            serial_number: Some("SRCH-1".into()),
            origin: Some(Origin::Imported),
            year_of_manufacture: Some(1943),
            country_of_manufacture: Some("Belgium".into()),
            importer_name: Some("Global Arms Import Co.".into()),
            ..base_input()
        },
        false,
    )
    .unwrap();

    assert_eq!(search(&db.conn, "1943"), 1, "should match year of manufacture");
    assert_eq!(search(&db.conn, "Global Arms"), 1, "should match importer name");
    assert_eq!(search(&db.conn, "Belgium"), 1, "should match country of manufacture");
}

#[test]
fn searching_country_for_a_reimported_firearm_finds_united_states() {
    use hoplodex_lib::models::firearm::Origin;

    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            serial_number: Some("SRCH-2".into()),
            origin: Some(Origin::Reimported),
            importer_name: Some("Century International Arms".into()),
            ..base_input()
        },
        false,
    )
    .unwrap();

    assert_eq!(
        search(&db.conn, "United States"),
        1,
        "a re-imported firearm's country is displayed and searched as United States"
    );
}

// specs/002-firearm-identification US2-5: the original manufacturer's marks
// are searchable too.

#[test]
fn searching_the_original_serial_number_or_maker_finds_the_firearm() {
    use hoplodex_lib::models::firearm::Origin;

    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            serial_number: Some("SRCH-3".into()),
            origin: Some(Origin::Imported),
            original_make: Some("Fabrique Nationale".into()),
            original_model: Some("High Power".into()),
            original_serial_number: Some("FN-99001".into()),
            registration_class_id: None,
            registration_form: None,
            registration_approved: None,
            registered_to: None,
            ..base_input()
        },
        false,
    )
    .unwrap();

    assert_eq!(search(&db.conn, "FN-99001"), 1, "should match the original serial number");
    assert_eq!(search(&db.conn, "Fabrique Nationale"), 1, "should match the original maker");
}

// specs/002-firearm-identification research.md §6: the origin label exists
// in three places (the SQL CASE in 0002_fts5.sql, Origin::label(), and
// ORIGIN_OPTIONS in src/features/firearms/types.ts, checked by a separate
// Vitest test). This walks every Origin variant, searching by its exact
// `label()` text, so the SQL and Rust copies can't silently drift apart —
// if either changed without the other, the search would stop matching.
#[test]
fn every_origin_is_found_by_searching_its_own_label() {
    use hoplodex_lib::models::firearm::Origin;

    let db = TestDb::new();
    for (i, origin) in
        [Origin::Domestic, Origin::Imported, Origin::Reimported].into_iter().enumerate()
    {
        ops::create_firearm(
            &db.conn,
            &FirearmInput {
                serial_number: Some(format!("LABEL-{i}")),
                origin: Some(origin),
                ..base_input()
            },
            false,
        )
        .unwrap();
    }

    for origin in [Origin::Domestic, Origin::Imported, Origin::Reimported] {
        assert_eq!(
            search(&db.conn, origin.label()),
            if origin == Origin::Imported { 2 } else { 1 },
            "searching {:?}'s own label() should find it via the SQL CASE (\"imported\" also \
             matches \"Re-imported\")",
            origin
        );
    }
}

// specs/002-firearm-identification SC-006: at collection scale, a search on
// any of the seven new fields still finds exactly the one firearm carrying
// it, not a false positive from the rest of the collection.
#[test]
fn a_search_on_any_new_field_finds_exactly_the_one_firearm_carrying_it_among_500() {
    use hoplodex_lib::models::firearm::Origin;

    let db = TestDb::new();
    const FILLER_COUNT: usize = 499;
    for i in 0..FILLER_COUNT {
        ops::create_firearm(
            &db.conn,
            &FirearmInput {
                make: format!("Filler Make {i}"),
                model: format!("Filler Model {i}"),
                serial_number: Some(format!("FILLER-{i}")),
                ..base_input()
            },
            false,
        )
        .unwrap();
    }

    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            make: "Distinctive Make".into(),
            model: "Distinctive Model".into(),
            serial_number: Some("DISTINCTIVE-MAIN".into()),
            origin: Some(Origin::Imported),
            year_of_manufacture: Some(1601),
            country_of_manufacture: Some("Ruritania".into()),
            importer_name: Some("Uniquestar Imports LLC".into()),
            original_make: Some("Zzyzx Arms".into()),
            original_model: Some("Model Zeta".into()),
            original_serial_number: Some("ZZ-999999".into()),
            registration_class_id: None,
            registration_form: None,
            registration_approved: None,
            registered_to: None,
            ..base_input()
        },
        false,
    )
    .unwrap();

    let total: usize = ops::list_firearms(&db.conn, &ListFirearmsInput::default())
        .unwrap()
        .groups
        .iter()
        .map(|g| g.firearms.len())
        .sum();
    assert_eq!(total, FILLER_COUNT + 1, "sanity: every record was actually saved");

    for (query, what) in [
        ("Imported", "origin"),
        ("1601", "year of manufacture"),
        ("Ruritania", "country of manufacture"),
        ("Uniquestar Imports LLC", "importer name"),
        ("Zzyzx Arms", "original maker"),
        ("Model Zeta", "original model"),
        ("ZZ-999999", "original serial number"),
    ] {
        assert_eq!(search(&db.conn, query), 1, "searching {what} ({query:?}) among 500 records");
    }
}

/// specs/004-cartridges-action-types FR-008 / US1-9: the cartridge is
/// searchable, a partly typed designation included.
#[test]
fn matches_the_cartridge_including_a_partial_designation_and_a_custom_word() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &FirearmInput { cartridge: Some("7.62x39mm".into()), ..base_input() },
        false,
    )
    .unwrap();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            cartridge: Some("Zyxwv Wildcat Special".into()),
            serial_number: Some("WILD-1".into()),
            ..base_input()
        },
        false,
    )
    .unwrap();

    assert_eq!(search(&db.conn, "7.62x39"), 1, "a partial cartridge designation");
    assert_eq!(search(&db.conn, "7.62x39mm"), 1, "the whole cartridge");
    assert_eq!(search(&db.conn, "Wildcat"), 1, "a word of a custom cartridge");
}

/// specs/004-cartridges-action-types FR-020 / US3-5: the action's name is
/// searchable, and follows the record when the action changes.
#[test]
fn matches_the_action_name_and_follows_a_change_of_action() {
    let db = TestDb::new();
    let bolt = ops::create_firearm(
        &db.conn,
        &FirearmInput {
            firearm_type_id: 2,
            action_type_id: Some(3),
            serial_number: Some("AC-1".into()),
            ..base_input()
        },
        false,
    )
    .unwrap();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            firearm_type_id: 2,
            action_type_id: Some(1),
            serial_number: Some("AC-2".into()),
            ..base_input()
        },
        false,
    )
    .unwrap();

    assert_eq!(search(&db.conn, "bolt"), 1, "a word of the action name");
    assert_eq!(search(&db.conn, "Bolt action"), 1, "the whole action name");
    assert_eq!(search(&db.conn, "semi"), 1, "a partly typed action name");

    ops::update_firearm(
        &db.conn,
        bolt.id,
        &FirearmInput { action_type_id: Some(4), ..FirearmInput::from(&bolt) },
        false,
    )
    .unwrap();
    assert_eq!(search(&db.conn, "bolt"), 0, "the old action's name no longer finds it");
    assert_eq!(search(&db.conn, "lever"), 1, "the new one does");
}

/// specs/005-regulated-item-types US3: the new action's name is searchable.
#[test]
fn matches_the_automatic_or_select_fire_action() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            firearm_type_id: 2,
            action_type_id: Some(13),
            serial_number: Some("SF-1".into()),
            ..base_input()
        },
        false,
    )
    .unwrap();
    assert_eq!(search(&db.conn, "select-fire"), 1);
    assert_eq!(search(&db.conn, "automatic"), 1);
}

/// specs/005-regulated-item-types US2-11, FR-017: the classification, form
/// and "Registered to" are searched; the approved date is not.
#[test]
fn registration_is_found_by_classification_form_and_registered_to() {
    let db = TestDb::new();
    let add = |serial: &str, class: i64, form: Option<&str>, to: Option<&str>| {
        ops::create_firearm(
            &db.conn,
            &FirearmInput {
                serial_number: Some(serial.into()),
                registration_class_id: Some(class),
                registration_form: form.map(str::to_owned),
                registration_approved: Some("2026-02-10".into()),
                registered_to: to.map(str::to_owned),
                ..base_input()
            },
            false,
        )
        .unwrap()
    };
    let trust = add("RG-1", 1, Some("Form 4"), Some("Smith Family Trust"));
    add("RG-2", 2, Some("Form 10"), Some("QZ Holdings"));
    add("RG-3", 3, Some("Form 1"), None);
    ops::create_firearm(
        &db.conn,
        &FirearmInput { serial_number: Some("PLAIN".into()), ..base_input() },
        false,
    )
    .unwrap();

    assert_eq!(search(&db.conn, "smith family"), 1);
    assert_eq!(search(&db.conn, "form 1"), 2, "Form 1 and Form 10");
    assert_eq!(search(&db.conn, "short-barreled"), 2, "rifle and shotgun");
    assert_eq!(search(&db.conn, "short-barreled rifle"), 1);
    // One and two characters go through the LIKE branch.
    assert_eq!(search(&db.conn, "qz"), 1);
    assert_eq!(search(&db.conn, "q"), 1);
    assert_eq!(search(&db.conn, "2026-02-10"), 0, "the approved date is not searched");

    ops::update_firearm(
        &db.conn,
        trust.id,
        &FirearmInput { registered_to: Some("Jones Estate".into()), ..FirearmInput::from(&trust) },
        false,
    )
    .unwrap();
    assert_eq!(search(&db.conn, "smith family"), 0);
    assert_eq!(search(&db.conn, "jones estate"), 1);
    ops::update_firearm(
        &db.conn,
        trust.id,
        &FirearmInput {
            registration_class_id: None,
            registration_form: None,
            registration_approved: None,
            registered_to: None,
            ..FirearmInput::from(&trust)
        },
        false,
    )
    .unwrap();
    assert_eq!(search(&db.conn, "jones estate"), 0);
    assert_eq!(search(&db.conn, "form 4"), 0);
}
