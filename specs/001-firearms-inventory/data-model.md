# Phase 1 Data Model: Firearms Collection Inventory

Derived from the Key Entities section of [spec.md](./spec.md) and the
functional requirements (FR-001–FR-036). All tables live in the single
encrypted SQLCipher database described in [research.md](./research.md).
Every table below is a real SQL table (or virtual table for FTS5) — none of
this is mocked for testing; `cargo test` integration tests run against a
real temporary SQLCipher database with this schema applied.

## Entity: FirearmType (lookup)

Structured, extensible list backing FR-012's grouping and the generic
thumbnail requirement (FR-009).

| Field | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `name` | TEXT, unique, not null | e.g. "Handgun", "Rifle", "Shotgun", "Other" |
| `generic_thumbnail_key` | TEXT, not null | key into bundled app-resource images (research.md §10) |

Seeded at first run with Handgun/Rifle/Shotgun/Other per spec Assumptions;
users may add new types later (out of scope for this feature to expose a
management UI beyond what's needed to satisfy FR-012's "at minimum type,
caliber, make").

## Entity: Firearm

Primary record; corresponds directly to the spec's **Firearm** entity.

| Field | Type | Notes / validation |
|---|---|---|
| `id` | INTEGER PK | |
| `make` | TEXT, not null | structured field (FR-012, grouping) |
| `model` | TEXT, not null | structured field |
| `serial_number` | TEXT, nullable | FR-001, FR-029: null only if `no_serial_attested = true` |
| `no_serial_attested` | BOOLEAN, not null, default false | FR-029: explicit attestation; save is blocked in the app layer if `serial_number IS NULL AND no_serial_attested = false` |
| `nickname` | TEXT, nullable | FR-031: optional free-form label to tell similar records apart; blank input stored as null; unique (case-insensitive, trimmed) among `status = 'active'` rows only, so a disposed firearm releases it; not part of the identifying key (FR-030), not used in import matching; indexed by FTS5; not a grouping field |
| `caliber` | TEXT, not null | structured field |
| `firearm_type_id` | INTEGER FK → FirearmType, not null | structured field |
| `notes` | TEXT, nullable | free-form (FR-002), indexed by FTS5 |
| `accessories` | TEXT, nullable | free-form list (FR-002), stored as delimited text or JSON array, indexed by FTS5 |
| `status` | TEXT, not null, default 'active' | enum: `active`, `disposed` (FR-023, FR-025) |
| `estimated_value` | INTEGER (cents), nullable | FR-005; null/0 treated as "no value set" for warning purposes (Edge Cases) |
| `acquisition_source` | TEXT, nullable | FR-003 |
| `acquisition_date` | TEXT (ISO 8601 date), nullable | FR-003 |
| `acquisition_price` | INTEGER (cents), nullable | FR-003 |
| `disposition_type` | TEXT, nullable | enum: `sold`, `traded`, `gifted`, `destroyed`, `lost_stolen`; required when `status = disposed` (FR-004) |
| `disposition_recipient` | TEXT, nullable | FR-004 |
| `disposition_date` | TEXT (ISO 8601 date), nullable | FR-004 |
| `disposition_price` | INTEGER (cents), nullable | FR-004 |
| `thumbnail_photo_id` | INTEGER FK → Photo, nullable | FR-008; null ⇒ use FirearmType's generic thumbnail (FR-009) |
| `insurance_policy_id` | INTEGER FK → InsurancePolicy, nullable | FR-014, FR-027: set only when the firearm is individually scheduled under that policy; null ⇒ unscheduled, covered by the blanket policy in force (computed, never stored per firearm, FR-036) |
| `scheduled_coverage_amount` | INTEGER (cents), nullable | FR-014, FR-024: set iff `insurance_policy_id` is set |
| `created_at` / `updated_at` | TEXT (ISO 8601 datetime), not null | audit trail, also drives "last updated" ordering if needed |

**Validation rules** (enforced in `services::firearms` / command layer, not
just at the DB level, so import validation (FR-020) can produce per-row
human-readable errors):

- `serial_number IS NOT NULL XOR no_serial_attested = true` must not both be
  false (Acceptance Scenario US1.7).
- `status = disposed` requires all four disposition fields set
  (Acceptance Scenario US1.4); `status = active` requires them null.
- `insurance_policy_id` and `scheduled_coverage_amount` are either both set
  (individually scheduled) or both null (unscheduled). Blanket coverage is
  never stored on the firearm; see Insurance Coverage below (FR-036).
- **Dates (FR-003, FR-004)**: `acquisition_date` and `disposition_date`,
  when set, must not be later than the user's current local date (today is
  allowed); `disposition_date` must not be earlier than `acquisition_date`
  when both are set. Both may be null. Checked on create, update, dispose,
  and every import row.
- **Nickname uniqueness (FR-031)**: `nickname` must not equal (ignoring
  case and surrounding whitespace) the nickname of any other
  `status = 'active'` firearm. Backstop: partial unique index on
  `nickname COLLATE NOCASE WHERE status = 'active' AND nickname IS NOT NULL`.
- **Identity uniqueness (FR-032)**: among other `status = 'active'`
  firearms, compare `(make, model, serial_number)` ignoring case and
  surrounding whitespace, only when `serial_number IS NOT NULL`. Match and
  `no_serial_attested = false` → `VALIDATION_ERROR` (blocked). Match and
  `no_serial_attested = true` → save succeeds and the response carries a
  warning. Matches against disposed rows are ignored. Backstop for the
  blocking case only: partial unique index on
  `(make, model, serial_number)` with NOCASE collation
  `WHERE status = 'active' AND no_serial_attested = 0 AND serial_number IS NOT NULL`.
  The app is unreleased, so no compatibility with earlier development
  databases is required; the migration may create the index directly.
- The two uniqueness checks are also re-run when a disposition is reversed
  (a disposed record returning to `active` may now clash with a record
  created in the meantime) and on every import row.
- Deleting a Firearm cascades to delete its Photos, DocumentAttachments, and DispositionHistory rows
  (spec Assumption: "Deleting a firearm record also removes its
  uniquely-associated photographs and document attachments").

**State transitions**: `active → disposed` (dispose, FR-004) and
`disposed → active` (reverse disposition, FR-033). Reversal requires the
user's explicit choice of `keep` or `discard` for the current disposition:
- `keep`: insert one `DispositionHistory` row copied from
  `disposition_type/recipient/date/price`, then null the four `Firearm`
  disposition columns.
- `discard`: null the four columns; nothing is stored.
Both set `status = 'active'` in the same transaction, after re-running the
nickname and make/model/serial checks (FR-031, FR-032) against currently
active firearms; a clash aborts the whole reversal with nothing changed.
Because the `Firearm` columns always hold only the *current* disposition,
the rule "`status = active` requires them null" is unchanged. Disposal is
never a deletion (FR-023). Deletion is a distinct, separate action
available from either state (FR-006), requiring confirmation.

## Entity: DispositionHistory

Retained past dispositions of a firearm that was restored to active
(FR-033). Written only by `reverse_disposition` with `keep`; never edited.

| Field | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `firearm_id` | INTEGER FK → Firearm, not null, on delete cascade | real deletion with the firearm (constitution V) |
| `disposition_type` | TEXT, not null | same enum as `Firearm.disposition_type` |
| `disposition_recipient` | TEXT, not null | |
| `disposition_date` | TEXT (ISO 8601 date), not null | |
| `disposition_price` | INTEGER (cents), nullable | |
| `reversed_at` | TEXT (ISO 8601 datetime), not null | when the user reversed it |

Not indexed by FTS5 and not included in the spreadsheet export/import (one
row per firearm; see contracts/spreadsheet-format.md).

## Entity: Photo

| Field | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `firearm_id` | INTEGER FK → Firearm, not null, on delete cascade | |
| `original_bytes` | BLOB, not null | original file, in its original format (for export, FR-018) |
| `original_filename` | TEXT, not null | preserved for export |
| `mime_type` | TEXT, not null | |
| `thumbnail_bytes` | BLOB, not null | pre-generated small JPEG (research.md §4) |
| `sort_order` | INTEGER, not null | order added; first photo (`sort_order = 0`) is the default thumbnail candidate (FR-008) |
| `created_at` | TEXT (ISO 8601 datetime), not null | |

`Firearm.thumbnail_photo_id` is the source of truth for *which* photo is
the thumbnail; `sort_order = 0` is only the default at insert time (US4
Acceptance Scenario 1–2).

## Entity: DocumentAttachment

| Field | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `firearm_id` | INTEGER FK → Firearm, not null, on delete cascade | |
| `file_bytes` | BLOB, not null | e.g. PDF receipt/appraisal (FR-010) |
| `original_filename` | TEXT, not null | |
| `mime_type` | TEXT, not null | |
| `created_at` | TEXT (ISO 8601 datetime), not null | |

## Entity: InsurancePolicy

| Field | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `name` | TEXT, not null | user-assigned (FR-027) |
| `policy_number` | TEXT, not null | |
| `insurance_company` | TEXT, not null | |
| `company_contact` | TEXT, nullable | phone/email/address, free text |
| `agent_name` | TEXT, nullable | |
| `agent_contact` | TEXT, nullable | |
| `blanket_coverage_limit` | INTEGER (cents), nullable | FR-027, FR-036: set ⇒ this is a blanket policy, and the limit is shared by all unscheduled firearms while the policy is in force; null ⇒ schedule-only policy |
| `effective_start_date` | TEXT (ISO 8601 date), not null | |
| `effective_end_date` | TEXT (ISO 8601 date), not null | drives 30-day and expired warnings (FR-028) |
| `created_at` / `updated_at` | TEXT (ISO 8601 datetime), not null | |

**Validation rules**:

- `effective_end_date > effective_start_date`.
- **At most one blanket policy in force (FR-036)**: for any two policies with
  a non-null `blanket_coverage_limit`, `A.start < B.end AND B.start < A.end`
  must be false, so their dates may share one boundary day (`A.end = B.start`)
  but no more. Checked on create and on every update that changes a policy's
  dates or blanket limit. Enforced in the command/service layer, since SQLite
  has no range-exclusion constraint.
- Deleting a policy that has firearms scheduled under it (FR-034) requires
  the caller to resolve them first, in the same transaction as the deletion:
  `move` sets `insurance_policy_id` to the chosen target policy on every
  affected firearm and leaves `scheduled_coverage_amount` unchanged;
  `unschedule` sets both `insurance_policy_id` and
  `scheduled_coverage_amount` to null. `unschedule` on a non-expired policy
  additionally needs an explicit confirmation flag. The foreign key stays
  `ON DELETE RESTRICT` as a backstop so a bug can never silently un-insure
  firearms; an unresolved deletion fails with `POLICY_HAS_FIREARMS`. Blanket
  firearms need no handling: their coverage is computed.

**Derived status** (not stored, computed by `services::insurance_status`):

- `is_expired`: `effective_end_date < today`.
- `is_expiring_soon`: `effective_end_date` within 30 days of today and not
  yet expired.
- `is_in_force`: `effective_start_date <= today <= effective_end_date`.
- `blanket_in_force`: the policy with a non-null `blanket_coverage_limit`
  that `is_in_force` (on a shared boundary day, the one with the later
  `effective_start_date`); null if there is none.
- FR-028 warning suppression for blanket policies: no expiring warning while
  another blanket policy starts no later than the day after this one ends;
  no expired warning once a blanket policy with a later start date exists.

## Entity: Insurance Coverage (derived, not a table)

Only *individually-scheduled* coverage is a per-firearm fact, stored as
`Firearm.insurance_policy_id` + `scheduled_coverage_amount` (one-to-many,
since FR-027 allows each firearm at most one scheduled policy). *Blanket*
coverage is never stored per firearm: `services::insurance_status` computes
it from policy dates (FR-036), so adding a firearm, renewing a policy, or
replacing one never touches firearm rows.

- **Individually-scheduled coverage**: `scheduled_coverage_amount` vs.
  `Firearm.estimated_value` (FR-024, US3 Acceptance Scenario 2–3). Uninsured
  if the policy `is_expired` (US3 Acceptance Scenario 8).
- **Blanket coverage**: let `B` be `blanket_in_force`. `SUM(estimated_value)`
  of every *active* firearm with `insurance_policy_id IS NULL` is compared
  against `B.blanket_coverage_limit` (FR-017, FR-024, US3 Acceptance Scenario
  4). If there is no `B`, every active unscheduled firearm is uninsured
  (US3 Acceptance Scenario 13). Disposed firearms never count (FR-025).

## Virtual table: firearms_fts (FTS5)

External-content FTS5 table over `Firearm`, kept in sync via `AFTER INSERT
/ UPDATE / DELETE` triggers, indexing: `make`, `model`, `nickname`, `serial_number`,
`caliber`, `notes`, `accessories`, and the joined `FirearmType.name`.
Satisfies FR-013 (search across all recorded information including
free-form notes) and US2 Acceptance Scenarios 3–4.

## Entity relationships (summary)

```text
FirearmType (1) ──< (many) Firearm
Firearm (1) ──< (many) Photo
Firearm (1) ──< (many) DocumentAttachment
Firearm (1) ──< (many) DispositionHistory
Firearm (1) ── thumbnail_photo_id ──> (1) Photo            [nullable]
InsurancePolicy (1) ──< (many) Firearm  [insurance_policy_id, nullable]
Firearm ──< firearms_fts (FTS5 shadow index, external-content)
```

## Import row shape (not persisted as an entity)

Export/import spreadsheets (FR-018/019, [contracts/spreadsheet-format.md](./contracts/spreadsheet-format.md))
use one row per Firearm with all `Firearm` fields above (minus photo/
document BLOBs, per FR-019's "without photographs") plus resolved
`insurance_policy` fields by name/number for readability. Import matching
(FR-026, FR-030) keys on `(make, model, serial_number)` when
`no_serial_attested = false`; rows with `no_serial_attested = true` are
always treated as new inserts, never matched.
