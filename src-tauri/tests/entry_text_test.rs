//! specs/004-cartridges-action-types research.md §3 and §9: the entry key
//! that decides which values are same-notation variants (FR-012, FR-013),
//! and the entry rules for make, model, cartridge and caliber (FR-015).

use hoplodex_lib::services::entry_text::{
    EntryField, check_entry_text, entry_key, initialism, words,
};

/// Pairs the spec calls same-notation variants: equal keys.
const VARIANTS: &[(&str, &str)] = &[
    ("Smith & Wesson", "smith and wesson"),
    ("Smith & Wesson", "Smith&Wesson"),
    ("Smith & Wesson", "SMITH & WESSON"),
    ("Smith & Wesson", "Smith and  Wesson"),
    ("9 x 19mm parabellum", "9×19mm Parabellum"),
    ("9x19mm Parabellum", "9 x 19mm parabellum"),
    ("9mm", "9 mm"),
    ("9x19", "9 x 19"),
    ("9x19", "9×19"),
    (".30-30 Winchester", "30-30 Winchester"),
    (".30-30 Winchester", ".30\u{2013}30 Winchester"),
    (".30-06 Springfield", ".30\u{2014}06 springfield"),
    ("Springfield Armory", "springfield armory"),
    ("Springfield Armory", "Springfield-Armory"),
    ("10/22", "10 22"),
    ("Mini-14", "mini 14"),
    ("Heckler & Koch", "Heckler and Koch"),
    // NFKC folds full-width forms.
    ("9mm", "\u{FF19}\u{FF4D}\u{FF4D}"),
];

/// Pairs the spec calls different notations: different keys.
const DIFFERENT: &[(&str, &str)] = &[
    ("9mm", "9x19"),
    ("9mm", "9x19mm Parabellum"),
    ("9x19", "9x19mm Parabellum"),
    ("S&W", "Smith & Wesson"),
    (".22 LR", ".22 Long Rifle"),
    ("9mm +P", "9mm P"),
    ("Česká", "Ceska"),
    ("Xtreme", "treme"),
    ("Wax", "Wa"),
    (".45 ACP", ".45 GAP"),
    ("Colt's", "Colts"),
];

#[test]
fn same_notation_variants_have_equal_keys() {
    for (a, b) in VARIANTS {
        assert_eq!(entry_key(a), entry_key(b), "{a:?} and {b:?} are variants");
    }
}

#[test]
fn different_notations_have_different_keys() {
    for (a, b) in DIFFERENT {
        assert_ne!(entry_key(a), entry_key(b), "{a:?} and {b:?} are different notations");
    }
}

#[test]
fn keys_are_the_joined_tokens() {
    assert_eq!(entry_key("Smith & Wesson"), "smithwesson");
    assert_eq!(entry_key("smith and wesson"), "smithwesson");
    assert_eq!(entry_key("Smith&Wesson"), "smithwesson");
    assert_eq!(entry_key("SMITH & WESSON"), "smithwesson");
    assert_eq!(entry_key("9 x 19mm parabellum"), "919mmparabellum");
    assert_eq!(entry_key("9×19mm Parabellum"), "919mmparabellum");
    assert_eq!(entry_key("9x19"), "919");
    assert_eq!(entry_key(".30-30 Winchester"), "3030winchester");
    assert_eq!(entry_key(".22 LR"), "22lr");
    assert_eq!(entry_key(".22 Long Rifle"), "22longrifle");
    assert_eq!(entry_key("S&W"), "sw");
    // An x that is not between two digits is a letter.
    assert_eq!(entry_key("Xtreme"), "xtreme");
    assert_eq!(entry_key("Wax"), "wax");
    assert_eq!(entry_key("Wax 12"), "wax12");
    assert_eq!(entry_key("12 xl 3"), "12xl3");
    // `and` is dropped only as a whole token.
    assert_eq!(entry_key("Anderson"), "anderson");
    assert_eq!(entry_key("Sandy"), "sandy");
    assert_eq!(entry_key(""), "");
    assert_eq!(entry_key("  &  "), "");
}

#[test]
fn words_split_on_separators_and_drop_and() {
    assert_eq!(words("Smith & Wesson"), vec!["smith", "wesson"]);
    assert_eq!(words("smith and wesson"), vec!["smith", "wesson"]);
    assert_eq!(words(".22 Long Rifle"), vec!["22", "long", "rifle"]);
    assert_eq!(words("9x19mm Parabellum"), vec!["9", "19mm", "parabellum"]);
    assert_eq!(words(".30-06 Springfield"), vec!["30", "06", "springfield"]);
}

#[test]
fn initialisms_take_the_first_character_of_each_word() {
    assert_eq!(initialism("Smith & Wesson"), "sw");
    assert_eq!(initialism("Heckler & Koch"), "hk");
    assert_eq!(initialism("Heckler and Koch"), "hk");
    assert_eq!(initialism("Springfield Armory"), "sa");
    assert_eq!(initialism("Ruger"), "r");
}

#[test]
fn entry_rules_cap_length_at_100_characters_after_trimming() {
    let hundred = "a".repeat(100);
    assert_eq!(check_entry_text(EntryField::Make, &hundred), Ok(()));
    assert_eq!(check_entry_text(EntryField::Make, &format!("  {hundred}  ")), Ok(()));
    assert_eq!(
        check_entry_text(EntryField::Make, &"a".repeat(101)),
        Err("Make can be at most 100 characters.".to_owned())
    );
    // Counted as characters (Unicode scalar values), not bytes.
    assert_eq!(check_entry_text(EntryField::Model, &"é".repeat(100)), Ok(()));
    assert_eq!(
        check_entry_text(EntryField::Cartridge, &"é".repeat(101)),
        Err("Cartridge can be at most 100 characters.".to_owned())
    );
    assert_eq!(
        check_entry_text(EntryField::Caliber, &"9".repeat(101)),
        Err("Caliber can be at most 100 characters.".to_owned())
    );
}

#[test]
fn entry_rules_refuse_control_characters() {
    assert_eq!(
        check_entry_text(EntryField::Make, "Smith\tWesson"),
        Err("Make can't contain control characters.".to_owned())
    );
    assert_eq!(
        check_entry_text(EntryField::Model, "19\u{7}"),
        Err("Model can't contain control characters.".to_owned())
    );
    assert_eq!(
        check_entry_text(EntryField::Cartridge, "9mm\nLuger"),
        Err("Cartridge can't contain control characters.".to_owned())
    );
    // Surrounding whitespace, tabs included, is trimmed first.
    assert_eq!(check_entry_text(EntryField::Make, "\tGlock\n"), Ok(()));
}

#[test]
fn make_model_and_caliber_are_required_but_cartridge_is_not() {
    assert_eq!(check_entry_text(EntryField::Make, "   "), Err("Make is required.".to_owned()));
    assert_eq!(check_entry_text(EntryField::Model, ""), Err("Model is required.".to_owned()));
    assert_eq!(check_entry_text(EntryField::Caliber, " "), Err("Caliber is required.".to_owned()));
    assert_eq!(check_entry_text(EntryField::Cartridge, "  "), Ok(()));
    assert_eq!(check_entry_text(EntryField::Cartridge, ""), Ok(()));
}

#[test]
fn fields_serialize_in_lower_case() {
    assert_eq!(serde_json::to_string(&EntryField::Cartridge).unwrap(), "\"cartridge\"");
    assert_eq!(serde_json::from_str::<EntryField>("\"make\"").unwrap(), EntryField::Make);
    assert!(serde_json::from_str::<EntryField>("\"nickname\"").is_err());
}
