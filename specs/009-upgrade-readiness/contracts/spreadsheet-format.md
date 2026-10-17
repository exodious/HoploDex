# Contract: Spreadsheet Format (feature 009 delta)

A delta against the format as 006's and 008's contracts left it. The columns
are unchanged: this feature adds no column to either table (FR-003).

## `insurance_policy_name` (FR-006; research.md §2)

- **Import** looks the cell up among the database's policies by the rule that
  makes policy names unique (FR-007). The cell and each policy's name are
  compared after removing surrounding spaces (U+0020), with the letters A–Z
  matched in either case. Other letters must match exactly, so "Église" and
  "église" are different names.
- **One match**: the row's item is scheduled under that policy, with the
  row's `scheduled_coverage_amount`.
- **No match**: the row error `Unknown insurance policy: <cell>`, and nothing
  from that row is saved (unchanged).
- **Never more than one match**, since no two policies can have names that
  compare equal.
- **Export** writes the policy's name as stored, unchanged.

The policy table itself, and how an imported policy row is matched, stay
#54's.
