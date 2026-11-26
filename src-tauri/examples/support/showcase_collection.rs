//! The showcase collection: "Main collection" under `human_seed --showcase`,
//! which `npm run screenshots:showcase` shoots for the GitHub Pages site and
//! the README (docs/development/screenshots.md, "The showcase").
//!
//! Small and made to be looked at, unlike the human-testing collection, which
//! exercises every rule and edge case:
//! - every firearm and accessory has a real photo from `seed-photos/`, and
//!   only public domain or CC0 ones, which the site's footer says (checked by
//!   `tests/human_seed_coverage_test.rs`);
//! - handguns, rifles, a shotgun and a suppressor, so the collection's first
//!   screen mixes them;
//! - the policies' dates are fixed, so no screen depends on the day: two run
//!   until the end of 2028 and one ended in 2024, which leaves one firearm
//!   under-insured and one uninsured. From December 2028 the in-force pair
//!   is "expiring", and the showcase spec refuses to shoot until these dates
//!   move on.

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::firearm::{Condition, FirearmInput};
use hoplodex_lib::models::insurance_policy::InsurancePolicyInput;
use hoplodex_lib::models::record::RecordRef;
use rusqlite::Connection;

use super::main_collection::*;

/// When the policies in force start and end: fixed, so the insurance page
/// reads the same on any day before December 2028.
const IN_FORCE: (&str, &str) = ("2026-01-01", "2028-12-31");
/// The rider that ended, for good: its firearm is uninsured.
const ENDED: (&str, &str) = ("2024-01-01", "2024-12-31");

/// Seeds the showcase collection into `conn`, a new database.
pub fn seed_showcase(conn: &Connection) {
    let policy = |name: &str, number: &str, limit: Option<i64>, (start, end): (&str, &str)| {
        let input = InsurancePolicyInput {
            name: name.into(),
            policy_number: number.into(),
            insurance_company: if limit.is_some() {
                "Cascade Mutual"
            } else {
                "Heritage Fine Arts"
            }
            .into(),
            company_contact: text("1-800-555-0142"),
            agent_name: text("Pat Alvarez"),
            agent_contact: text("pat.alvarez@example.com"),
            notes: None,
            blanket_coverage_limit: limit,
            effective_start_date: start.into(),
            effective_end_date: end.into(),
        };
        must(insurance_ops::create_policy(conn, &input), name).id
    };
    let collector = policy("Collector Schedule", "CS-771204", None, IN_FORCE);
    policy("Homeowners Blanket", "HB-2026-90311", Some(10_000), IN_FORCE);
    let rider = policy("Range Bag Rider", "RB-30918", None, ENDED);

    let add = |input: FirearmInput, photos: &[(&str, Vec<u8>)]| {
        let label = format!("{} {}", input.make, input.model);
        let id = must(firearm_ops::create_firearm(conn, &input, false, None), &label).id;
        for (name, bytes) in photos {
            must(photo_ops::add_photo(conn, RecordRef::Firearm(id), bytes, name), name);
        }
        id
    };
    let add_accessory = |input: AccessoryInput, (name, bytes): (&str, Vec<u8>)| {
        let label = format!("{} {}", input.make, input.model);
        let id = must(accessory_ops::create_accessory(conn, &input, None), &label).id;
        must(photo_ops::add_photo(conn, RecordRef::Accessory(id), &bytes, name), name);
    };

    // -- Handguns, under the blanket policy ---------------------------------

    add(
        FirearmInput {
            nickname: text("Daily"),
            notes: text("Trigger job done by a gunsmith in 2022."),
            estimated_value: Some(550),
            acquisition_source: text("Ridgeline Arms"),
            acquisition_date: text("2021-03-14"),
            acquisition_price: Some(529),
            barrel_length_hundredths: Some(402),
            overall_length_hundredths: Some(740),
            weight_tenths_oz: Some(236),
            capacity: Some(15),
            finish: text("Black nDLC"),
            condition: Some(Condition::Excellent),
            cartridge: text("9x19mm Parabellum"),
            action_type_id: Some(SEMI_AUTOMATIC),
            ..base("Glock", "19 Gen5", "BXKT482", "9mm", HANDGUN)
        },
        &[
            seed_photo!("glock-19-gen3.jpg"),
            seed_photo!("glock-19-gen4-fde.jpg"),
            seed_photo!("glock-19-atf.jpg"),
        ],
    );
    add(
        FirearmInput {
            estimated_value: Some(650),
            acquisition_date: text("2019-05-18"),
            cartridge: text("9x19mm Parabellum"),
            action_type_id: Some(SEMI_AUTOMATIC),
            ..base("Beretta", "92FS", "BER556213", "9mm", HANDGUN)
        },
        &[seed_photo!("beretta-92fs-atf.jpg")],
    );
    let springfield = add(
        FirearmInput {
            estimated_value: Some(750),
            acquisition_date: text("2016-08-20"),
            cartridge: text(".45 ACP"),
            action_type_id: Some(SEMI_AUTOMATIC),
            ..base("Springfield Armory", "1911-A1 Mil-Spec", "SA-911073", ".45", HANDGUN)
        },
        &[seed_photo!("1911a1-field-stripped.jpg")],
    );

    // -- Collector pieces, scheduled individually ---------------------------

    add(
        FirearmInput {
            notes: text("Original, not a replica. Appraised in 2025."),
            estimated_value: Some(2_800),
            acquisition_source: text("Estate sale"),
            acquisition_date: text("2018-10-06"),
            insurance_policy_id: Some(collector),
            scheduled_coverage_amount: Some(3_000),
            action_type_id: Some(PERCUSSION),
            ..base("Remington", "New Model Army", "RNA-31406", ".44", HANDGUN)
        },
        &[seed_photo!("remington-new-model-army.jpg")],
    );
    add(
        FirearmInput {
            estimated_value: Some(3_200),
            acquisition_date: text("2020-04-11"),
            insurance_policy_id: Some(collector),
            scheduled_coverage_amount: Some(3_500),
            cartridge: text(".44-40 Winchester"),
            action_type_id: Some(LEVER_ACTION),
            ..base("Winchester", "Model 1873", "W73-58411", ".44", RIFLE)
        },
        &[seed_photo!("winchester-1873.jpg")],
    );
    // Scheduled for less than its value: under-insured.
    add(
        FirearmInput {
            estimated_value: Some(1_900),
            acquisition_date: text("2012-06-02"),
            insurance_policy_id: Some(collector),
            scheduled_coverage_amount: Some(1_500),
            cartridge: text(".30-06 Springfield"),
            action_type_id: Some(SEMI_AUTOMATIC),
            ..base("Springfield Armory", "M1 Garand", "3416208", ".30", RIFLE)
        },
        &[
            seed_photo!("m1-garand-left.jpg"),
            seed_photo!("m1-garand-right.jpg"),
            seed_photo!("m1-garand-receiver.jpg"),
            seed_photo!("m1-garand-stock.jpg"),
            seed_photo!("m1-garand-sling.jpg"),
        ],
    );

    // -- A rifle with its optics, and a shotgun on a rider that ended -------

    let colt = add(
        FirearmInput {
            estimated_value: Some(1_100),
            acquisition_date: text("2022-11-25"),
            cartridge: text("5.56x45mm NATO"),
            action_type_id: Some(SEMI_AUTOMATIC),
            ..base("Colt", "LE6920", "LE-334519", ".22", RIFLE)
        },
        &[seed_photo!("m4-carbine-ras.jpg")],
    );
    // Uninsured: scheduled only on the rider that ended.
    let remington = add(
        FirearmInput {
            estimated_value: Some(500),
            acquisition_date: text("2015-09-19"),
            insurance_policy_id: Some(rider),
            scheduled_coverage_amount: Some(500),
            cartridge: text("12 gauge"),
            action_type_id: Some(PUMP_ACTION),
            ..base("Remington", "870 Tactical", "RS48213K", "12 gauge", SHOTGUN)
        },
        &[seed_photo!("remington-870-mcs.jpg")],
    );

    // -- A registered suppressor, on the 1911, with its paperwork -----------

    let osprey = add(
        FirearmInput {
            estimated_value: Some(650),
            acquisition_source: text("Ridgeline Arms"),
            acquisition_date: text("2025-09-12"),
            acquisition_price: Some(650),
            registration_class_id: Some(REGISTERED_SUPPRESSOR),
            registration_form: text("Form 4"),
            registration_approved: text("2026-02-10"),
            registered_to: text("Smith Family Trust"),
            mounted_on: Some(RecordRef::Firearm(springfield)),
            ..base("SilencerCo", "Osprey 45", "OSP-45-20817", ".45", SUPPRESSOR)
        },
        &[seed_photo!("silencerco-osprey-45.jpg")],
    );
    for (name, bytes) in [
        (
            "Form 4 approval.pdf",
            simple_pdf(&[&["Approved Form 4", "Registered to Smith Family Trust"]]),
        ),
        (
            "Bill of sale (scan).tif",
            include_bytes!("../../tests/fixtures/documents/showcase-bill-of-sale-scan.tif")
                .to_vec(),
        ),
    ] {
        must(document_ops::add_document(conn, RecordRef::Firearm(osprey), &bytes, name), name);
    }

    // -- Accessories, each on the firearm it's mounted on -------------------

    let on = |host: i64, kind: i64, make: &str, model: &str, value: i64| AccessoryInput {
        estimated_value: Some(value),
        mounted_on: Some(RecordRef::Firearm(host)),
        ..bare_accessory(kind, make, model)
    };
    add_accessory(
        on(colt, KIND_OPTIC, "Trijicon", "ACOG TA31", 1_400),
        seed_photo!("trijicon-m150-rco.jpg"),
    );
    add_accessory(on(colt, KIND_LIGHT, "Steiner", "DBAL-A2", 1_500), seed_photo!("peq-15a.jpg"));
    add_accessory(
        on(remington, KIND_OPTIC, "Aimpoint", "CompM4", 800),
        seed_photo!("aimpoint-compm4.jpg"),
    );
    add_accessory(
        AccessoryInput {
            estimated_value: Some(80),
            cartridge: text("5.56x45mm NATO"),
            caliber: text(".22"),
            ..bare_accessory(KIND_MAGAZINE, "Colt", "30-round magazines (4)")
        },
        seed_photo!("epm-magazine.jpg"),
    );
}
