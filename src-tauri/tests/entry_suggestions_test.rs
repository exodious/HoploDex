//! specs/004-cartridges-action-types User Story 2 through `ops`, against a
//! real temporary SQLCipher database: the ranked suggestion list for make,
//! model, cartridge and caliber (research.md §4), and `settle_entry`'s
//! snapping of same-notation variants (research.md §6).

mod support;

use hoplodex_lib::commands::entries::{
    SettleEntryInput, SettleEntryOutput, SuggestEntriesInput, Suggestion, ops as entry_ops,
};
use hoplodex_lib::commands::firearms::{DisposeFirearmInput, ops};
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput};
use hoplodex_lib::services::cartridges::{CaliberSource, catalog};
use hoplodex_lib::services::entry_text::EntryField;
use support::{TestDb, firearm};

/// A record of `make` and `model` with the given cartridge and caliber,
/// each on a serial number of its own.
fn record(db: &TestDb, make: &str, model: &str, cartridge: Option<&str>, caliber: &str) -> i64 {
    let count: i64 = db.conn.query_row("SELECT COUNT(*) FROM firearms", [], |r| r.get(0)).unwrap();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            cartridge: cartridge.map(str::to_owned),
            caliber: caliber.into(),
            ..firearm(make, model, &format!("S-{count}"))
        },
        false,
    )
    .unwrap()
    .id
}

fn suggest_with(db: &TestDb, field: EntryField, text: &str, make: Option<&str>) -> Vec<Suggestion> {
    entry_ops::suggest_entries(
        &db.conn,
        &SuggestEntriesInput { field, text: text.into(), make: make.map(str::to_owned) },
    )
    .unwrap()
    .suggestions
}

fn suggest(db: &TestDb, field: EntryField, text: &str) -> Vec<Suggestion> {
    suggest_with(db, field, text, None)
}

fn values(suggestions: &[Suggestion]) -> Vec<&str> {
    suggestions.iter().map(|s| s.value.as_str()).collect()
}

fn position(suggestions: &[Suggestion], value: &str) -> usize {
    suggestions
        .iter()
        .position(|s| s.value == value)
        .unwrap_or_else(|| panic!("{value:?} is not in {:?}", values(suggestions)))
}

fn settle(db: &TestDb, field: EntryField, text: &str) -> SettleEntryOutput {
    entry_ops::settle_entry(&db.conn, &SettleEntryInput { field, text: text.into() }).unwrap()
}

fn makes_db() -> TestDb {
    let db = TestDb::new();
    record(&db, "Smith & Wesson", "686", None, ".357");
    record(&db, "Heckler & Koch", "USP", None, "9mm");
    record(&db, "Ruger", "10/22", None, ".22");
    db
}

#[test]
fn makes_are_found_by_prefix_word_and_initialism() {
    // US2-1.
    let db = makes_db();
    for typed in ["sw", "s&w", "smith w", "smith and w", "wesson"] {
        assert!(
            values(&suggest(&db, EntryField::Make, typed)).contains(&"Smith & Wesson"),
            "{typed:?} should find Smith & Wesson"
        );
    }
    assert!(values(&suggest(&db, EntryField::Make, "hk")).contains(&"Heckler & Koch"));
    assert_eq!(values(&suggest(&db, EntryField::Make, "r")), ["Ruger"]);
}

#[test]
fn a_cartridge_is_found_by_its_leading_digits_with_or_without_the_dot() {
    // US2-2.
    let db = TestDb::new();
    for typed in ["30", ".30"] {
        let list = suggest(&db, EntryField::Cartridge, typed);
        let found = values(&list);
        assert!(found.contains(&".30-30 Winchester"), "{typed:?}: {found:?}");
        assert!(found.contains(&".30 Carbine"), "{typed:?}: {found:?}");
        assert!(!found.contains(&"9x19mm Parabellum"), "{typed:?}: {found:?}");
    }
    for typed in ["3006", "30-06"] {
        assert!(
            values(&suggest(&db, EntryField::Cartridge, typed)).contains(&".30-06 Springfield")
        );
    }
    let found = suggest(&db, EntryField::Cartridge, "22");
    for name in [".22 Long Rifle", ".22 Short", ".22 WMR"] {
        assert!(values(&found).contains(&name), "{name:?} missing from {:?}", values(&found));
    }
}

#[test]
fn a_bore_class_lists_its_cartridges_most_common_first() {
    // US2-3.
    let db = TestDb::new();
    let list = suggest(&db, EntryField::Cartridge, "9mm");
    let order: Vec<usize> = ["9x19mm Parabellum", ".380 ACP", "9x18mm Makarov", "9x21mm"]
        .iter()
        .map(|name| position(&list, name))
        .collect();
    assert_eq!(order, [0, 1, 2, 3], "{:?}", values(&list));

    let settled = settle(&db, EntryField::Cartridge, "9mm");
    assert_eq!(settled.value, "9mm");
    assert_eq!(settled.changed_by, None);
    let derived = settled.derived_caliber.unwrap();
    assert_eq!((derived.caliber.as_str(), derived.source), ("9mm", CaliberSource::Guess));
}

#[test]
fn aliases_narrow_the_list_and_similar_names_stay_distinct() {
    let db = TestDb::new();
    for (typed, name) in [
        (".22 LR", ".22 Long Rifle"),
        ("9mm Luger", "9x19mm Parabellum"),
        ("9mm Para", "9x19mm Parabellum"),
        ("9x19", "9x19mm Parabellum"),
    ] {
        let list = suggest(&db, EntryField::Cartridge, typed);
        assert!(values(&list).contains(&name), "{typed:?} should find {name:?}");
    }
    assert!(values(&suggest(&db, EntryField::Cartridge, ".223")).contains(&".223 Remington"));
    assert!(values(&suggest(&db, EntryField::Cartridge, "5.56")).contains(&"5.56x45mm NATO"));
    assert_ne!(
        values(&suggest(&db, EntryField::Cartridge, ".223"))[0],
        values(&suggest(&db, EntryField::Cartridge, "5.56"))[0]
    );
}

#[test]
fn a_value_in_the_catalog_and_on_record_appears_once_with_both_markers() {
    // US2-4.
    let db = TestDb::new();
    record(&db, "Glock", "17", Some("9x19mm Parabellum"), "9mm");
    record(&db, "Glock", "19", Some("9X19MM PARABELLUM"), "9mm");
    record(&db, "Glock", "26", Some("9x19mm parabellum"), "9mm");

    let list = suggest(&db, EntryField::Cartridge, "9x19");
    let matching: Vec<&Suggestion> =
        list.iter().filter(|s| s.value.to_lowercase() == "9x19mm parabellum").collect();
    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0].value, "9x19mm Parabellum", "the catalog's spelling");
    assert!(matching[0].in_catalog);
    assert_eq!(matching[0].use_count, 3);
    assert_eq!(matching[0].caliber.as_deref(), Some("9mm"));

    let calibers = suggest(&db, EntryField::Caliber, "9");
    let nine = calibers.iter().find(|s| s.value == "9mm").unwrap();
    assert!(nine.in_catalog);
    assert_eq!(nine.use_count, 3);
    assert_eq!(nine.caliber, None);

    let makes = suggest(&db, EntryField::Make, "glo");
    assert_eq!(makes.len(), 1);
    assert!(!makes[0].in_catalog);
    assert_eq!(makes[0].use_count, 3);
}

#[test]
fn suggestions_are_ordered_and_capped() {
    let db = TestDb::new();
    // Record values come before catalog-only ones, by use count.
    record(&db, "Glock", "17", Some(".45 ACP"), ".45");
    record(&db, "Glock", "21", Some(".45 ACP"), ".45");
    record(&db, "Colt", "1911", Some(".45 Colt"), ".45");
    record(&db, "Colt", "Cobra", Some(".45 Wild"), ".45");
    let list = suggest(&db, EntryField::Cartridge, ".45");
    assert_eq!(&values(&list)[..3], [".45 ACP", ".45 Colt", ".45 Wild"]);
    assert!(list[3..].iter().all(|s| s.use_count == 0), "{:?}", values(&list));
    // A tie in use count goes to the more common (lower-ranked) value, and
    // catalog-only values run in rank order within a tier: those that start
    // with .45 or are of the .45 class are tier 1, the rest (5.56x45mm NATO,
    // a later word) come after.
    let tier_one: Vec<&Suggestion> = list[3..]
        .iter()
        .take_while(|s| {
            let entry = catalog().entries().iter().find(|e| e.name == s.value).unwrap();
            entry.caliber == ".45" || entry.name.starts_with(".45")
        })
        .collect();
    assert!(tier_one.len() >= 5, "{:?}", values(&list));
    let ranks: Vec<u32> = tier_one
        .iter()
        .map(|s| catalog().entries().iter().find(|e| e.name == s.value).unwrap().rank)
        .collect();
    assert!(ranks.windows(2).all(|w| w[0] <= w[1]), "{ranks:?}");

    // Tier 1 (a prefix) comes before tier 2 (a later word), before tier 3.
    let db = TestDb::new();
    record(&db, "Alpha Sw", "1", None, "9mm");
    record(&db, "Sw Beta", "2", None, "9mm");
    record(&db, "Smith & Wesson", "3", None, "9mm");
    record(&db, "Zed", "4", None, "9mm");
    let list = suggest(&db, EntryField::Make, "sw");
    assert_eq!(values(&list), ["Sw Beta", "Alpha Sw", "Smith & Wesson"]);

    // Empty text: the most used values first, then the most common catalog
    // entries; never more than 20.
    let db = TestDb::new();
    record(&db, "Glock", "17", Some(".40 S&W"), ".40");
    record(&db, "Glock", "22", Some(".40 S&W"), ".40");
    record(&db, "Colt", "1911", Some("Rare Wildcat"), ".45");
    let list = suggest(&db, EntryField::Cartridge, "");
    assert_eq!(list.len(), 20);
    assert_eq!(&values(&list)[..3], [".40 S&W", "Rare Wildcat", "9x19mm Parabellum"]);
    assert_eq!(values(&list)[3], ".22 Long Rifle");
    assert_eq!(suggest(&db, EntryField::Make, "").len(), 2);
}

#[test]
fn a_bore_class_is_offered_for_the_caliber_field() {
    let db = TestDb::new();
    let list = suggest(&db, EntryField::Caliber, "12");
    assert!(values(&list).contains(&"12 gauge"));
    assert!(list.iter().all(|s| s.caliber.is_none()));
}

#[test]
fn text_over_100_characters_matches_nothing() {
    let db = makes_db();
    assert!(suggest(&db, EntryField::Make, &"x".repeat(101)).is_empty());
    assert!(suggest(&db, EntryField::Cartridge, &"9".repeat(101)).is_empty());
}

#[test]
fn a_variant_of_a_make_snaps_to_the_spelling_on_record() {
    // US2-5.
    let db = makes_db();
    for typed in ["smith and wesson", "Smith&Wesson", "SMITH & WESSON", " smith  &  wesson "] {
        let settled = settle(&db, EntryField::Make, typed);
        assert_eq!(settled.value, "Smith & Wesson", "{typed:?}");
        assert_eq!(
            settled.changed_by,
            Some(hoplodex_lib::services::suggestions::ChangedBy::Record)
        );
    }
    let same = settle(&db, EntryField::Make, "Smith & Wesson");
    assert_eq!((same.value.as_str(), same.changed_by), ("Smith & Wesson", None));
    let other = settle(&db, EntryField::Make, "S&W");
    assert_eq!((other.value.as_str(), other.changed_by), ("S&W", None));
    let new = settle(&db, EntryField::Make, "  Taurus ");
    assert_eq!((new.value.as_str(), new.changed_by), ("Taurus", None));
}

#[test]
fn a_cartridge_variant_snaps_to_the_catalog_but_another_notation_is_kept() {
    use hoplodex_lib::services::suggestions::ChangedBy;
    // US2-6.
    let db = TestDb::new();
    for typed in ["9 x 19mm parabellum", "9×19mm Parabellum"] {
        let settled = settle(&db, EntryField::Cartridge, typed);
        assert_eq!(settled.value, "9x19mm Parabellum", "{typed:?}");
        assert_eq!(settled.changed_by, Some(ChangedBy::Catalog));
        let derived = settled.derived_caliber.unwrap();
        assert_eq!((derived.caliber.as_str(), derived.source), ("9mm", CaliberSource::Catalog));
    }
    for typed in ["9x19", "9mm Luger"] {
        let settled = settle(&db, EntryField::Cartridge, typed);
        assert_eq!((settled.value.as_str(), settled.changed_by), (typed, None));
        assert_eq!(settled.derived_caliber.unwrap().caliber, "9mm");
    }
}

#[test]
fn the_snap_target_is_the_catalog_then_the_most_used_record_spelling() {
    use hoplodex_lib::services::suggestions::ChangedBy;
    let db = TestDb::new();
    // Two spellings on record; the more used wins, the lowest id on a tie.
    record(&db, "Springfield armory", "XD", None, "9mm");
    record(&db, "Springfield Armory", "XDM", None, "9mm");
    let tie = settle(&db, EntryField::Make, "springfield  armory");
    assert_eq!(tie.value, "Springfield armory", "a tie goes to the lowest id");
    record(&db, "Springfield Armory", "Hellcat", None, "9mm");
    let most_used = settle(&db, EntryField::Make, "SPRINGFIELD ARMORY");
    assert_eq!(most_used.value, "Springfield Armory");
    assert_eq!(most_used.changed_by, Some(ChangedBy::Record));

    // The catalog wins over a record spelling.
    record(&db, "Glock", "17", Some("9X19mm Parabellum"), "9mm");
    record(&db, "Glock", "19", Some("9X19mm Parabellum"), "9mm");
    let cartridge = settle(&db, EntryField::Cartridge, "9x19MM parabellum");
    assert_eq!(cartridge.value, "9x19mm Parabellum");
    assert_eq!(cartridge.changed_by, Some(ChangedBy::Catalog));

    // Models snap across makes.
    record(&db, "Colt", "Python", None, ".357");
    let model = settle(&db, EntryField::Model, "PYTHON");
    assert_eq!((model.value.as_str(), model.changed_by), ("Python", Some(ChangedBy::Record)));
}

#[test]
fn a_derived_caliber_is_itself_snapped_against_calibers() {
    let db = TestDb::new();
    // A bore the catalog has no class for, spelled two ways on record.
    record(&db, "Custom", "One", Some(".19 Calhoon"), "19");
    record(&db, "Custom", "Two", Some(".19 Calhoon"), "19");
    record(&db, "Custom", "Three", Some(".19 Calhoon"), ".19");
    let settled = settle(&db, EntryField::Cartridge, ".19 Calhoon");
    let derived = settled.derived_caliber.expect("a bore can be read");
    assert_eq!(derived.caliber, "19", "snapped to the caliber most used on record");
    assert_eq!(derived.source, CaliberSource::Guess);
}

#[test]
fn a_value_disappears_with_its_only_firearm_but_not_when_it_is_disposed() {
    // US2-7.
    let db = TestDb::new();
    let deleted = record(&db, "Ruger", "Wildcat", Some("Wildcat Special"), ".30");
    let disposed = record(&db, "Ruger", "Bobcat", Some("Bobcat Special"), ".30");
    assert_eq!(values(&suggest(&db, EntryField::Cartridge, "wildcat")), ["Wildcat Special"]);
    assert_eq!(values(&suggest(&db, EntryField::Model, "wildcat")), ["Wildcat"]);

    ops::delete_firearm(&db.conn, deleted, true).unwrap();
    assert!(suggest(&db, EntryField::Cartridge, "wildcat").is_empty());
    assert!(suggest(&db, EntryField::Model, "wildcat").is_empty());
    assert_eq!(settle(&db, EntryField::Cartridge, "WILDCAT SPECIAL").changed_by, None);

    ops::dispose_firearm(
        &db.conn,
        disposed,
        &DisposeFirearmInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane".into(),
            date: "2025-01-01".into(),
            price: 100,
        },
    )
    .unwrap();
    assert_eq!(values(&suggest(&db, EntryField::Cartridge, "bobcat")), ["Bobcat Special"]);
    assert_eq!(values(&suggest(&db, EntryField::Model, "bobcat")), ["Bobcat"]);
}

#[test]
fn a_value_saved_only_as_a_pending_draft_is_not_suggested() {
    // US2-8.
    let db = TestDb::new();
    db.conn
        .execute(
            "INSERT INTO pending_changes
                 (id, kind, mode, target_id, label, form_version, values_json, saved_at)
             VALUES (1, 'firearm', 'add', NULL, 'Draft', 2,
                     '{\"make\":\"Draftmake\",\"cartridge\":\"Draft Round\"}',
                     '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
    assert!(suggest(&db, EntryField::Make, "draft").is_empty());
    assert!(suggest(&db, EntryField::Cartridge, "draft").is_empty());
}

#[test]
fn models_are_offered_only_for_the_make_on_the_form() {
    // US2-9, US2-11.
    let db = TestDb::new();
    record(&db, "Marlin", "336", None, ".30");
    record(&db, "Marlin", "336", None, ".30");
    record(&db, "Ruger", "10/22", None, ".22");
    record(&db, "Ruger", "Mini-14", None, ".22");
    record(&db, "Sig Sauer", "P365 XL", None, "9mm");

    let with_ruger = suggest_with(&db, EntryField::Model, "", Some("Ruger"));
    assert_eq!(values(&with_ruger), ["10/22", "Mini-14"]);
    let variant = suggest_with(&db, EntryField::Model, "", Some("  RUGER "));
    assert_eq!(values(&variant), ["10/22", "Mini-14"], "the make is compared by key");
    // Typing narrows within the make.
    let typed = suggest_with(&db, EntryField::Model, "mini", Some("Ruger"));
    assert_eq!(values(&typed), ["Mini-14"]);
    assert!(suggest_with(&db, EntryField::Model, "p3", Some("Ruger")).is_empty());

    // A make that is on no record has no models to offer.
    assert!(suggest_with(&db, EntryField::Model, "", Some("Glock")).is_empty());
    assert!(suggest_with(&db, EntryField::Model, "p365", Some("Glock")).is_empty());

    // No make yet (or only spaces): every model, most used first.
    for make in [None, Some(""), Some("  ")] {
        let all = suggest_with(&db, EntryField::Model, "", make);
        assert_eq!(values(&all), ["336", "10/22", "Mini-14", "P365 XL"], "{make:?}");
    }
}

#[test]
fn a_model_shared_by_two_makes_is_offered_for_each() {
    let db = TestDb::new();
    record(&db, "Colt", "Python", None, ".357");
    record(&db, "Other", "Python", None, ".357");
    record(&db, "Colt", "Cobra", None, ".38");

    let colt = suggest_with(&db, EntryField::Model, "", Some("Colt"));
    assert_eq!(values(&colt), ["Python", "Cobra"]);
    let other = suggest_with(&db, EntryField::Model, "", Some("Other"));
    assert_eq!(values(&other), ["Python"]);
}

#[test]
fn a_make_is_ignored_for_the_other_fields() {
    let db = TestDb::new();
    record(&db, "Ruger", "10/22", Some("Wildcat Special"), ".22");
    let plain = suggest(&db, EntryField::Cartridge, "wild");
    let with_make = suggest_with(&db, EntryField::Cartridge, "wild", Some("Glock"));
    assert_eq!(values(&with_make), values(&plain));
    assert_eq!(values(&plain), ["Wildcat Special"]);
}

/// The 25 most common cartridges, each findable by some 1 to 4 characters of
/// its name or an alias, in the first 8 suggestions of an empty collection,
/// and settling it derives its catalog caliber (SC-001).
#[test]
fn the_25_most_common_cartridges_are_found_within_four_characters() {
    let db = TestDb::new();
    for entry in &catalog().entries()[..25] {
        let found = std::iter::once(&entry.name).chain(&entry.aliases).any(|spelling| {
            let chars: Vec<char> = spelling.chars().collect();
            (1..=chars.len().min(4)).any(|len| {
                let prefix: String = chars[..len].iter().collect();
                let list = suggest(&db, EntryField::Cartridge, &prefix);
                list.iter().take(8).any(|s| s.value == entry.name)
            })
        });
        assert!(
            found,
            "{:?} is not in the first 8 for any prefix of up to 4 characters",
            entry.name
        );

        let derived = settle(&db, EntryField::Cartridge, &entry.name).derived_caliber.unwrap();
        assert_eq!(
            (derived.caliber.as_str(), derived.source),
            (entry.caliber.as_str(), CaliberSource::Catalog)
        );
    }
}

/// SC-003: 20 makes and cartridges on record; every case, spacing, separator
/// and `&`/`and` variant settles to the recorded spelling, and a different
/// notation never changes.
#[test]
fn variants_of_twenty_recorded_values_all_snap_and_other_notations_never_do() {
    let db = TestDb::new();
    let makes = [
        "Smith & Wesson",
        "Heckler & Koch",
        "Sturm, Ruger & Co.",
        "Cobra Firearms",
        "Springfield Armory",
        "Taurus",
        "Kel-Tec",
        "Bond Arms",
        "Rock Island Armory",
        "Century Arms",
    ];
    let cartridges = [
        "Wildcat Special",
        ".300 Whisper",
        "7mm-08 Custom",
        "6mm ARC Improved",
        ".458 SOCOM Long",
        "5.45x39mm Custom",
        "9x19 Hot Load",
        "12/70 Slug",
        ".50 Beowulf Short",
        "10.4 x 38 Swiss",
    ];
    for (i, (make, cartridge)) in makes.iter().zip(cartridges).enumerate() {
        record(&db, make, &format!("M{i}"), Some(cartridge), "9mm");
    }
    for make in makes {
        for variant in [
            make.to_uppercase(),
            make.to_lowercase(),
            make.replace(" & ", " and "),
            make.replace(" & ", "&"),
            make.replace(' ', "  "),
        ] {
            let settled = settle(&db, EntryField::Make, &variant);
            assert_eq!(settled.value, make, "{variant:?}");
        }
    }
    for cartridge in cartridges {
        for variant in [cartridge.to_uppercase(), cartridge.replace(' ', "  ")] {
            assert_eq!(settle(&db, EntryField::Cartridge, &variant).value, cartridge);
        }
    }
    // Different notations are different values.
    let db2 = TestDb::new();
    record(&db2, "Smith & Wesson", "M", Some("9x19mm Parabellum"), "9mm");
    for kept in ["S&W", "Smith Wesson Co", "SW"] {
        assert_eq!(settle(&db2, EntryField::Make, kept).changed_by, None, "{kept:?}");
    }
    for kept in ["9mm", "9x19", "9mm Luger", "919 Parabellum"] {
        assert_eq!(settle(&db2, EntryField::Cartridge, kept).changed_by, None, "{kept:?}");
    }
}

/// A record registered as a Suppressor with the given form and owner.
fn registered(db: &TestDb, form: Option<&str>, to: Option<&str>) -> i64 {
    let count: i64 = db.conn.query_row("SELECT COUNT(*) FROM firearms", [], |r| r.get(0)).unwrap();
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            registration_class_id: Some(1),
            registration_form: form.map(str::to_owned),
            registered_to: to.map(str::to_owned),
            ..firearm("Make", "Model", &format!("R-{count}"))
        },
        false,
    )
    .unwrap()
    .id
}

#[test]
fn form_suggests_the_built_in_names_first_then_those_on_record() {
    // specs/005-regulated-item-types US2-5, FR-009.
    let db = TestDb::new();
    for typed in ["F", "Form"] {
        let found = suggest(&db, EntryField::RegistrationForm, typed);
        assert_eq!(
            values(&found),
            ["Form 4", "Form 1", "Form 3", "Form 5", "Form 10"],
            "typed {typed:?}"
        );
        assert!(found.iter().all(|s| s.in_catalog && s.use_count == 0));
    }
    // As everywhere (004 research.md §4), a value in use ranks above one
    // never used, and the built-in names keep their own order.
    registered(&db, Some("Form 4 eFiled"), None);
    registered(&db, Some("eForm 4"), None);
    let found = suggest(&db, EntryField::RegistrationForm, "Form");
    assert_eq!(
        values(&found),
        ["Form 4 eFiled", "Form 4", "Form 1", "Form 3", "Form 5", "Form 10"]
    );
    assert!(!found[0].in_catalog);
    assert!(found[1..].iter().all(|s| s.in_catalog));
    // A form on record matches from the start of any of its words.
    let found = suggest(&db, EntryField::RegistrationForm, "eF");
    assert_eq!(values(&found)[0], "eForm 4");
    let settled = settle(&db, EntryField::RegistrationForm, "form 4");
    assert_eq!(settled.value, "Form 4");
    assert_eq!(settled.changed_by, Some(hoplodex_lib::services::suggestions::ChangedBy::Catalog));
    assert!(settled.derived_caliber.is_none());
}

#[test]
fn registered_to_offers_only_values_on_record_and_snaps_to_them() {
    // FR-009, FR-013, SC-007.
    let db = TestDb::new();
    assert!(suggest(&db, EntryField::RegisteredTo, "").is_empty());
    let id = registered(&db, None, Some("Smith Family Trust"));
    registered(&db, None, Some("Only Deleted LLC"));
    let found = suggest(&db, EntryField::RegisteredTo, "smith");
    assert_eq!(values(&found), ["Smith Family Trust"]);
    assert!(!found[0].in_catalog);
    assert_eq!(found[0].caliber, None);

    let settled = settle(&db, EntryField::RegisteredTo, "smith family trust");
    assert_eq!(settled.value, "Smith Family Trust");
    assert_eq!(settled.changed_by, Some(hoplodex_lib::services::suggestions::ChangedBy::Record));
    assert!(settled.derived_caliber.is_none());

    let last: i64 = db
        .conn
        .query_row("SELECT id FROM firearms WHERE registered_to = 'Only Deleted LLC'", [], |r| {
            r.get(0)
        })
        .unwrap();
    ops::delete_firearm(&db.conn, last, true).unwrap();
    assert!(suggest(&db, EntryField::RegisteredTo, "only").is_empty());
    assert_eq!(values(&suggest(&db, EntryField::RegisteredTo, "")), ["Smith Family Trust"]);
    let _ = id;
}
