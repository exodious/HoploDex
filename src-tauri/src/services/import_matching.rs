//! Import matching/conflict-resolution key (FR-026, FR-030,
//! data-model.md's "Import row shape"): `(make, model, serial_number)`
//! when `no_serial_attested = false`; rows with `no_serial_attested = true`
//! are always treated as new inserts, never matched.

use rusqlite::{named_params, Connection, OptionalExtension};

use crate::commands::CommandError;

/// Returns the id of an existing firearm matching `(make, model,
/// serial_number)` ignoring letter case and surrounding whitespace (the same
/// comparison as FR-032), or `None` if there is no match or
/// `no_serial_attested` is set (a no-serial row can never match an existing
/// one, even if make/model coincide). When both an active and a disposed
/// record match, the active one is the match: it is the one FR-032 protects.
pub fn find_match(
    conn: &Connection,
    make: &str,
    model: &str,
    serial_number: Option<&str>,
    no_serial_attested: bool,
) -> Result<Option<i64>, CommandError> {
    if no_serial_attested {
        return Ok(None);
    }
    conn.query_row(
        "SELECT id FROM firearms
         WHERE lower(trim(make)) = lower(trim(:make))
           AND lower(trim(model)) = lower(trim(:model))
           AND lower(trim(serial_number)) = lower(trim(:serial_number))
         ORDER BY (status = 'active') DESC, id
         LIMIT 1",
        named_params! { ":make": make, ":model": model, ":serial_number": serial_number },
        |row| row.get(0),
    )
    .optional()
    .map_err(CommandError::from_db)
}
