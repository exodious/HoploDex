//! The one integration-test program (#77 step 9): every `tests/*_test.rs` file is a
//! module of it, so the crate links once instead of once per file. A new test file
//! needs a `mod` line here (`test_files_test` fails without one: `autotests` is off).
//!
//! `performance_test` and `import_largest_test` (release-only, `#[ignore]`d, run by
//! name), `macos_screen_lock_test` (its own `main`), `portability_test` and
//! `tiff_preview_test` (they change environment variables, which is sound only while
//! no other test's thread reads the environment, so under `cargo test` they mustn't
//! share a process with the rest), and `release_upgrade_test` (it keys with the
//! pinned KDF, which a `test-kdf` build can choose only for a whole process) stay
//! programs of their own.

#[path = "support/preview_support.rs"]
mod preview_support;
mod sheets;
mod support;

mod accessory_spreadsheet_test;
mod accessory_test;
mod acl_manifest_test;
mod action_type_test;
mod amount_cap_test;
mod app_version_test;
mod backup_due_tracking_test;
mod backup_location_change_test;
mod backup_test;
mod caliber_guess_test;
mod cartridge_catalog_test;
mod cartridge_test;
mod chooser_details_test;
mod csp_test;
mod database_open_test;
mod deletion_wipe_test;
mod disk_full_test;
mod disposition_reversal_test;
mod document_test;
mod document_types_test;
mod entry_suggestions_test;
mod entry_text_test;
mod export_test;
mod file_swap_test;
mod firearm_lifecycle_test;
mod fts_search_test;
mod human_seed_coverage_test;
mod identification_test;
mod identity_uniqueness_test;
mod import_binding_test;
mod import_export_test;
mod import_matching_test;
mod import_reader_test;
mod insurance_policy_test;
mod insurance_status_test;
mod keyring_test;
mod last_saved_version_test;
mod list_accessories_test;
mod list_firearms_test;
mod lock_held_test;
mod lock_test;
mod machine_settings_test;
mod mount_lifecycle_test;
mod mount_test;
mod nickname_test;
mod open_document_test;
mod original_marks_warning_test;
mod passphrase_change_test;
mod passphrase_protection_test;
mod pdf_preview_test;
mod pending_changes_test;
mod pending_resume_test;
mod photo_test;
mod physical_details_test;
mod policy_deletion_test;
mod portal_opener_test;
mod preview_text_test;
mod query_plan_test;
mod record_identifier_test;
mod registration_test;
mod released_migrations_test;
mod restore_test;
mod sandbox_reach_test;
mod scratch_ownership_test;
mod seed_sandbox_test;
mod serial_attestation_test;
mod session_scope_test;
mod session_test;
mod suppressor_test;
mod take_over_test;
mod test_files_test;
mod test_kdf_startup_test;
mod text_limits_test;
mod tiff_helper_test;
mod upgrade_steps_test;
mod upgrade_test;
mod valuation_test;
mod webkit_sandbox_test;
