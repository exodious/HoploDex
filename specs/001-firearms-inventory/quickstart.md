# Quickstart: Validate Firearms Collection Inventory

This is a runnable validation guide, not an implementation spec. It proves
the feature works end-to-end by walking through each user story's
independent test from spec.md. Field/command names reference
[data-model.md](./data-model.md) and [contracts/](./contracts).

## Prerequisites

- Rust toolchain (stable, 1.75+) with `cargo`.
- Node.js 18+ and a package manager (npm/pnpm) for the frontend.
- Tauri 2.x CLI prerequisites for your OS (WebView2 on Windows,
  WebKitGTK dev packages on Linux — see Tauri's own platform prerequisites).
- `tauri-driver` installed (for E2E) and the platform's WebDriver server
  available (WebView2 driver on Windows, `WebKitWebDriver` on Linux).

## Setup

```bash
# from repo root, once src-tauri/ and the frontend exist
npm install
cargo build --manifest-path src-tauri/Cargo.toml
```

First launch generates a random encryption key, stores it via `keyring` in
the OS credential store, creates the encrypted SQLCipher database in the
OS app-data directory, and applies migrations (schema + FTS5 tables +
seeded `FirearmType` rows).

```bash
npm run tauri dev
```

## Automated test commands

```bash
# Rust business logic + integration tests against a real temp SQLCipher DB
cargo test --manifest-path src-tauri/Cargo.toml

# Frontend unit tests
npm run test        # Vitest

# End-to-end acceptance-scenario tests (builds the app, drives it via tauri-driver)
npm run build:tauri
npm run test:e2e    # WebdriverIO against the built app
```

All three MUST pass before merge per the constitution's Development
Workflow gate.

## Manual / scripted validation per user story

### US1 — Record a Firearm (P1)

1. Launch the app with an empty collection.
2. Create a firearm with make, model, serial number, caliber, type → save
   → confirm it appears in the collection with those values intact
   (Acceptance Scenario 1).
3. Edit a field (add a note: "scratch on left side") → confirm it persists
   on reopen (Scenario 2).
   On a firearm with no notes or accessories, follow the "Add" link beside
   each empty list → the edit form opens scrolled to, focused on, and
   highlighting that field (Scenario 15).
4. Add acquisition details (source, date, price) → confirm shown on the
   record (Scenario 3). Type `1250.50` or `1,250` into the price field →
   the `.` and `,` are not accepted; paste `1250.50` → rejected with a
   whole-dollar message; enter `1250` → shown as "$1,250" (Scenario 16).
5. Mark it disposed (recipient, date, price, type) → confirm status flips
   to disposed while history remains (Scenario 4).
6. Delete a different firearm → confirm the confirmation prompt appears,
   then confirm removal (Scenario 5).
7. Create a firearm leaving serial number blank *and* checking "no serial
   number" attestation → save succeeds (Scenario 6).
8. Create a firearm leaving serial number blank *without* checking the
   attestation → save is blocked with a prompt (Scenario 7).
9. Give two firearms with identical make, model, and caliber different
   nicknames → both nicknames appear alongside make and model in list,
   tile, and detail views (Scenario 8).
10. Save another active firearm with an existing nickname (different
    letter case) → blocked, conflicting record named; dispose the first
    firearm → the nickname can now be used (Scenario 9).
11. Save a firearm with the same make, model, and serial as an
    active firearm → blocked; dispose the existing one → the same
    make/model/serial now saves as a new record (Scenario 10).
12. Check "no serial number" on a firearm with the same make and model as
    an active one → the serial field is unavailable and the save succeeds
    without being compared; uncheck it → a serial is required again
    (Scenario 11).
13. Reverse a disposition → asked whether to keep or discard the
    disposition details; firearm returns to active; kept details show as
    history; a clash with a current active firearm blocks the reversal
    (Scenario 12).
14. Enter a future acquisition date, then a future disposition date →
    each blocked; then a disposition date before the acquisition date →
    blocked; today's date is accepted (Scenarios 13-14).

### US2 — Browse, Search, and Group (P2)

1. Populate 5+ firearms spanning multiple types/calibers/makes, one with a
   distinctive note (e.g. "cracked handle").
2. Toggle list ↔ tile view → same firearms shown in both (Scenario 1).
3. Group by type → firearms bucket correctly (Scenario 2).
4. Search "cracked handle" → only that firearm returned (Scenario 3).
5. Search a caliber value shared by 2+ firearms → all of them returned
   (Scenario 4).
6. Clear search/group → full collection shown (Scenario 5).

### US3 — Track Value and Insurance Coverage (P3)

1. Create an `InsurancePolicy` with a blanket limit and an end date, with
   dates that do not yet include today.
2. Set one firearm's estimated value → "uninsured" warning shown, since it
   is unscheduled and no blanket policy is in force (Scenario 1); change the
   policy's dates to include today → the warning clears with no per-firearm
   assignment (Scenarios 11, 13).
3. Individually-schedule a firearm below its estimated value → "under-
   insured" warning (Scenario 2); raise the scheduled amount to meet/exceed
   value → warning disappears (Scenario 3).
4. Enter several unscheduled firearms whose combined value exceeds the
   blanket limit → group under-insured warning shown; nothing had to be
   assigned per firearm (Scenario 4).
5. Add/edit/dispose/delete a firearm → confirm `get_value_summary` total
   updates immediately with no manual refresh (Scenario 5).
6. View the value summary with a blanket policy in force, firearms
   scheduled under other policies, and some unscheduled firearms → confirm
   blanket total vs. limit + per-policy scheduled firearms + grand total
   (Scenario 6).
7. Set a blanket policy's end date to 15 days from today → expiration
   warning shown (Scenario 7); add a blanket policy starting the day after
   → the warning goes away (Scenario 14).
8. Set a policy's end date in the past → firearms scheduled under it (and
   unscheduled firearms, if it was the blanket policy in force) flagged
   uninsured in addition to the policy warning (Scenario 8).
9. Delete a non-expired policy that has firearms scheduled under it, with
   another policy available → asked to move them (or leave them unscheduled
   after a stronger confirmation stating the count and outcome); after
   choosing, warnings and the value summary update at once (Scenario 9).
10. Delete an expired policy that has scheduled firearms → warning that they
    are already uninsured; move or leave unscheduled, then it deletes
    (Scenario 10).
11. Create a second blanket policy overlapping the first by more than one
    day → blocked, conflicting policy named; make it start on the day the
    first ends → accepted (Scenario 12).
12. Delete the blanket policy currently in force → warned how many
    unscheduled firearms lose their coverage (Scenario 15).

### US4 — Attach Photos and Documents (P4)

1. Add a photo to a firearm with none → it becomes the thumbnail
   (Scenario 1).
2. Add a second photo, explicitly select it as thumbnail → browse views
   update (Scenario 2).
3. View a firearm with no photos → generic type thumbnail shown
   (Scenario 3).
4. Attach a PDF → reopen it from the record (Scenario 4).
5. Open the PDF in the external viewer, then quit the app → the app's
   `opened-documents` cache folder is gone; repeat but kill the app while
   the document is open → the next launch clears the folder (Scenario 5).

### US5 — Export and Import Records (P5)

1. Export the populated collection → inspect the produced spreadsheet
   (all fields present) and the sibling photos folder (Scenario 1).
2. Prepare a spreadsheet per
   [contracts/spreadsheet-format.md](./contracts/spreadsheet-format.md) and
   import it → confirm new/updated records appear (Scenario 2).
3. Include one row with a missing required field → confirm the import
   report names the failing row and reason, while other rows still import
   (Scenario 3).

## Success criteria checkpoints

Cross-reference results above against spec.md's SC-001 through SC-010
(time-to-add, search-at-500-records, always-fresh value summary,
100%-flagged warnings, single-action export, clear import error report,
universal thumbnails, zero network activity, 30-day policy warning, no decrypted document
copies left on disk).
