# Quickstart: validating feature 014

## Automated

Run through the dev container (`DEVELOPMENT.md`):

```sh
scripts/dev-container.sh bash -c 'npm run lint && npm test && cargo nextest run --manifest-path src-tauri/Cargo.toml --features test-kdf'
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/record-firearm.e2e.ts --spec e2e/specs/accessories-mounting.e2e.ts --spec e2e/specs/locking.e2e.ts --spec e2e/specs/value-insurance.e2e.ts'
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'
```

They cover each story's journey and the scenarios listed in [plan.md](plan.md#test-levels).

## Manual (best effort before a release)

**Setup**

1. Run `scripts/human-testing.sh`.
2. Open **Main collection** with the passphrase `human testing passphrase`.

**M1: markers, revert and Save on a firearm**

1. Open the Tikka T3x Lite "Deer Rifle" and choose **Edit**. Expected: Save looks disabled; hovering it says "No changes to save".
2. Change Make to `Tikka Oy`. Expected: "Changed" and ↺ beside Make; Save enabled.
3. Type Make back to `Tikka`. Expected: no marker; Save disabled.
4. Change Notes, then press ↺ beside Notes. Expected: Notes is back as saved, focus in Notes, Save disabled.
5. Change Type to Suppressor, then press ↺ beside Type. Expected: Type, Action, barrel length and capacity are back as saved, none marked.

**M2: closing with changes**

1. In the same form change Make, then press Escape. Expected: "Discard changes?" with focus on Keep editing.
2. Press Enter. Expected: the form is still open, Make still changed.
3. Press Escape, then choose Discard. Expected: the form closes; the record is unchanged.

**M3: Mounted list**

1. Open the Accessories page, group by Mounted on, and note an accessory mounted on a firearm (its host).
2. Open another active firearm's record page and choose **Change what's mounted…**. Expected: its edit form opens at the Mounted list.
3. Choose **Add…** and pick the accessory from step 1. Expected: a row marked Changed, saying it will move from its host; nothing has changed in the collection.
4. Press ↺ on that row. Expected: the row is gone; Save disabled.
5. Add it again and choose **Save**. Expected: the accessory is listed on this firearm's page and no longer on its old host's.

**M4: Settings**

1. Open **Database settings**. Expected: Save disabled, focusable, "No changes to save".
2. Tick **Remember the passphrase on this computer**. Expected: Changed and ↺ beside it; Save enabled.
3. Press ↺. Expected: unticked, Save disabled; nothing asked.
