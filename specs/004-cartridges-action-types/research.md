# Phase 0 Research: Cartridges, Action Types & Entry Suggestions

The Technical Context has no open unknowns: the stack is feature 001's and
no dependency is added. This document records the decisions the spec left
to planning (the catalog and its bore classes, FR-004a; the matching and
snapping rules in exact terms; where suggestions are computed) and the ones
the codebase forced (import reads columns by position today, §12), each
with its rationale, so nothing is left as NEEDS CLARIFICATION going into
Phase 1.

## 1. Where the cartridge catalog lives: a data file compiled into the backend

- **Decision**: The catalog is a tab-separated file,
  `src-tauri/src/services/cartridges/catalog.tsv`, embedded in the binary with
  `include_str!` and parsed once into a `OnceLock<Catalog>` on first use. It
  is never written to a database, has no table or migration, and no firearm
  refers to it. Each line is `rank<TAB>name<TAB>caliber<TAB>aliases`, aliases
  separated by `|`; `#` starts a comment line. The file's header comment
  states that it was written for HoploDex and is GPL-3.0-only like the rest
  of the source.
- **Rationale**: Every consumer of the catalog is in the backend: the
  suggestion list (§5), snapping (§6), caliber derivation (§7) and import
  (§12). One copy in one language means the form and import can never
  disagree about a spelling or a caliber. Embedding it (rather than a Tauri
  resource read from disk at run time) lets `cargo test` and
  `examples/human_seed.rs` use it without an `AppHandle`, keeps it read-only
  by construction, and is still "shipped with the application" as the
  recorded decision asks (it is the same kind of bundled, non-user data as the
  generic thumbnails, 001 research §10). A TSV diffs line by line, so a
  catalog change reads clearly in review.
- **Licensing (constitution, Licensing)**: The list is written for the
  project. Cartridge names are standard designations (the spellings SAAMI and
  C.I.P. publish are used as a reference for how a name is written, the way
  any reference book is), and no third-party dataset is copied. Recorded here
  and in the file's header, as the constitution requires for bundled data
  files.
- **Alternatives considered**: A seeded database table (rejected by the
  recorded decision: a catalog entry no firearm uses would be residue in the
  user's database, and constitution V's "deleted firearms leave nothing"
  would no longer hold by construction); a TypeScript module in the frontend
  (rejected: import runs in the backend and would need a second copy); a
  Rust `const` array (workable, but 250 struct literals are harder to review
  than 250 lines of text, and a data file keeps the door open for a
  generator later); JSON (rejected: noisier diffs, no comments).

## 2. Catalog content and bore classes (FR-002, FR-004a)

The spec settles that a catalog cartridge's caliber is its **bore class**,
not the finer designation in its name, and hands the class for each entry to
planning.

- **Decision — the principle**: A class is the nominal bore family collectors
  group by, named in the notation most collectors use for that family.
  Cartridges named in the other notation join the family they belong to, as
  the spec's own example does (`.380 ACP` → "9mm"; the issue's ".223 and
  5.56x45 are nominally the same caliber").
- **Decision — the class list** (spellings are exact, per the spec's
  Assumptions: inch classes with a leading dot, metric in millimetres with no
  space, shotguns by gauge or bore):

  | Class | Examples in the catalog |
  |---|---|
  | `.17` | .17 HMR, .17 WSM, .17 Hornet |
  | `.20` | .204 Ruger |
  | `.22` | .22 Short, .22 Long, .22 Long Rifle, .22 WMR, .22 Hornet, .222 Remington, .223 Remington, 5.56x45mm NATO, .22-250 Remington, .224 Valkyrie, .220 Swift, 5.7x28mm |
  | `6mm` | .243 Winchester, 6mm Creedmoor, 6mm ARC, 6mm Remington |
  | `.25` | .25 ACP, .25-06 Remington, .257 Roberts |
  | `6.5mm` | 6.5 Creedmoor, 6.5x55mm Swedish, 6.5 Grendel, 6.5 PRC, .260 Remington, .264 Winchester Magnum |
  | `.27` | .270 Winchester, .270 WSM, 6.8 SPC |
  | `7mm` | 7mm Remington Magnum, 7mm-08 Remington, 7x57mm Mauser, 7mm PRC, .280 Remington, .284 Winchester |
  | `.30` | .30 Carbine, .30-30 Winchester, .30-06 Springfield, .308 Winchester, .300 AAC Blackout, .300 Winchester Magnum, .300 WSM, .303 British, 7.62x39mm, 7.62x51mm NATO, 7.62x54mmR, 7.62x25mm Tokarev, 7.5x55mm Swiss, 7.7x58mm Arisaka |
  | `.32` | .32 ACP, .32 S&W Long, .327 Federal Magnum, .32-20 Winchester, .32 Winchester Special |
  | `8mm` | 8x57mm Mauser, 8mm Lebel, .325 WSM |
  | `.338` | .338 Winchester Magnum, .338 Lapua Magnum |
  | `.35` | .35 Remington, .35 Whelen, .358 Winchester |
  | `9.3mm` | 9.3x62mm, 9.3x74mmR |
  | `.357` | .38 Special, .357 Magnum, .38 S&W, .38 Short Colt, .38 Long Colt, .38 Super, .357 SIG |
  | `9mm` | 9x19mm Parabellum, .380 ACP, 9x18mm Makarov, 9x21mm, 9x23mm Winchester, 9mm Largo |
  | `.375` | .375 H&H Magnum, .375 Ruger |
  | `.40` | .40 S&W, 10mm Auto, .38-40 Winchester |
  | `.41` | .41 Remington Magnum |
  | `.44` | .44 Special, .44 Magnum, .44-40 Winchester |
  | `.45` | .45 ACP, .45 Colt, .45 GAP, .45-70 Government, .454 Casull, .460 S&W Magnum, .450 Bushmaster, .458 Winchester Magnum |
  | `.50` | .50 BMG, .500 S&W Magnum, .50 Action Express, .50 Beowulf |
  | `10 gauge`, `12 gauge`, `16 gauge`, `20 gauge`, `28 gauge`, `.410 bore` | the shotgun cartridge of the same name |

  The two cases the spec names: **7.62x39mm is ".30"** (cross-notation, like
  5.56 → ".22"; the spec's clarification puts every .30-class inch cartridge in
  ".30", and the metric 7.62s are the same bore). **.38 Special is ".357"**:
  it and .357 Magnum fire the same .357" bullet and a .357 revolver chambers
  both (spec Edge Cases), so they belong in one group, and ".357" is both the
  real bore and the class the spec's Assumptions list as an example.
- **Decision — size and rank**: roughly 250 entries: rimfire, handgun, rifle,
  shotgun and common military-surplus cartridges found in U.S. private
  collections (spec Assumptions). Rank 1 is the most common. The first 25,
  which SC-001 is measured on, are, in order: 9x19mm Parabellum, .22 Long
  Rifle, .223 Remington, 5.56x45mm NATO, 12 gauge, .45 ACP, .380 ACP, .40 S&W,
  .308 Winchester, .38 Special, .357 Magnum, 7.62x39mm, .30-06 Springfield,
  20 gauge, .300 AAC Blackout, .22 WMR, 6.5 Creedmoor, .243 Winchester, .270
  Winchester, .44 Magnum, 10mm Auto, .30-30 Winchester, .410 bore, 9x18mm
  Makarov, .45 Colt. This order also gives US2-3's "9x19, then .380, 9x18,
  9x21" for the 9mm class.
- **Decision — aliases**: the other names collectors write for an entry:
  ".22 LR" and "22LR" for .22 Long Rifle; "9mm Luger", "9mm Para", "9mm NATO"
  and "9x19" for 9x19mm Parabellum; ".300 BLK" and ".300 Blackout" for .300
  AAC Blackout; "5.56 NATO" and "5.56" for 5.56x45mm NATO; "7.65mm Browning"
  for .32 ACP; ".410" and "410 gauge" for .410 bore; "12 ga" for 12 gauge,
  and so on. An alias is never a class spelling (so a bare "9mm" typed as a
  cartridge stays a guess, US2-3).
- **Invariants, enforced by a unit test over the parsed file** (so a catalog
  edit that breaks one fails `cargo test`, not a user):
  1. Ranks are exactly 1..N with no gaps or repeats.
  2. No two names or aliases, across all entries, share an entry key (§3).
  3. No alias has the key of a class.
  4. Every caliber matches the class grammar (`^\.\d{2,3}$`, `^\d+(\.\d)?mm$`,
     `^\d{1,2} gauge$`, `^\.\d{3} bore$`) and is in the class list above.
  5. Entries whose names or aliases share a leading designation (§7) share a
     class, except for a short, named list of exceptions where the name's
     first number is not the bore (`.38-40 Winchester` is `.40` although
     `.38 Special` is `.357`; `7.65x53mm Argentine` is `.30` although
     `.32 ACP`'s alias `7.65mm Browning` is `.32`).
  6. For every entry, the guess (§7) applied to its name with " Improved"
     appended yields the entry's own class, unless the entry is one of the
     exceptions in 5. This ties the guess and the catalog together, so a
     custom variant of a known cartridge lands in the same group.
- **Alternatives considered**: The finer designation as caliber (".308",
  ".300") (rejected by the spec's clarification: broader groups); a
  diameter-only rule (rejected: .32 ACP and .30 rifle cartridges overlap in
  diameter but are grouped apart by every collector); ".38" for the .38
  Special family (rejected: .357 Magnum would then be ".38", which reads
  wrong to its owners, and the spec lists ".357"); a separate "10mm" class
  (rejected: it and .40 S&W share the .400" bore, and broader groups are the
  goal).

## 3. The entry key: what "same-notation variant" means, exactly (FR-012, FR-013)

Snapping, deduplication and matching all compare values through one
function, `entry_key`, in `services/entry_text.rs`.

- **Decision**: `entry_key(text)`:
  1. Unicode NFKC, then lower case (Unicode, not ASCII-only).
  2. Map `×` to `x`, and every dash (Unicode `Pd`) to `-`.
  3. Split into **tokens** on the separators: whitespace, `-`, `/`, `.`, `&`,
     and `x` when it stands between two digits (with or without spaces: `9x19`,
     `9 x 19`). Every other character, including `'`, `(`, `+`, `,` and `#`, is
     part of a token and significant.
  4. Drop tokens that are exactly `and`.
  5. Join the remaining tokens with nothing between them.

  Two values are **same-notation variants** when their keys are equal. So
  "Smith & Wesson", "smith and wesson", "Smith&Wesson" and "SMITH & WESSON" all
  have the key `smithwesson`; "9 x 19mm parabellum" and "9×19mm Parabellum"
  have `919mmparabellum`; "9mm" and "9 mm" have `9mm`. Different notations keep
  different keys: `9mm`, `919`, `919mmparabellum`; `sw` and `smithwesson`;
  `22lr` and `22longrifle`.
- **Words** (for word-prefix matching and initialisms) are the tokens of
  step 3 without `and`, before joining.
- **The leading dot** is a separator, so ".30-30 Winchester" and "30-30
  Winchester" are variants of each other, as FR-012 asks ("ignoring a leading
  dot").
- **Rationale**: The spec's rule, "differ only in letter case, spacing, the
  separators of FR-012, or `&` versus `and` between words", becomes a single
  function whose equality is the rule. Everything else (suggestion
  deduplication, snapping in the form, snapping on import, the sheet-majority
  pass) calls it, so they cannot drift apart. A table-driven unit test lists
  the spec's examples as variant and non-variant pairs (SC-003's "0
  different-notation values changed").
- **Alternatives considered**: Dropping all punctuation (rejected: widens
  "same notation" past what the spec lists, e.g. it would snap "9mm +P" to
  "9mm P"); folding diacritics (rejected: "Česká" and "Ceska" are different
  spellings, not a separator variant, and the spec never asks for it); treating
  every `x` as a separator (rejected: breaks words such as "Xtreme" or "Wax").

## 4. Matching and ranking suggestions (FR-010, FR-012, FR-016)

- **Decision — candidates**: For a field, the candidates are the field's values
  on record (all firearms, active and disposed) grouped by `entry_key`, merged
  with the catalog's values for that field: cartridge names for **cartridge**,
  class spellings for **caliber**, nothing for **make** and **model**. A
  candidate is shown with the catalog spelling when the catalog has its key,
  otherwise with the record spelling used by the most firearms (the earliest
  recorded, i.e. the lowest firearm id, on a tie; spec Edge Cases). Its
  **use count** is the number of firearms across all its spellings.
- **Decision — match tiers** for typed text `t` (all comparisons on keys and
  words from §3):
  - **Tier 1, prefix**: the candidate's key starts with `entry_key(t)`. ("30"
    and ".30" find ".30-30 Winchester", ".30 Carbine" and ".300 AAC Blackout";
    "3006" and "30-06" find ".30-06 Springfield".)
  - **Tier 2, word prefix**: `t` has two or more words, and each word of `t`
    is a prefix of a distinct word of the candidate, in order ("smith w",
    "long rifle"). A single typed word also matches any later word of the
    candidate ("wesson").
  - **Tier 3, initialism or catalog alias/class**: `entry_key(t)` has at least
    two characters and is a prefix of the candidate's initialism (the first
    character of each word: "sw" and "s&w" find Smith & Wesson, "hk" Heckler &
    Koch); or, for a catalog cartridge, one of its aliases matches `t` by tier
    1 to 3 (".22 LR", "9mm Luger"), or its class matches `t` by tier 1 ("9mm"
    finds every 9mm-class cartridge).
  A candidate takes its best tier.
- **Decision — order**: for **model**, candidates recorded with the make on the
  form (compared by key) come first, then the rest (US2-9). Then by tier; then
  candidates on record (use count > 0) before catalog-only; on record by use
  count, descending; catalog-only by rank; last, alphabetically by display
  spelling so the order is total and tests are stable.
- **Decision — empty text**: when the field is focused and empty, every
  candidate matches at tier 1, so the list opens with the user's most-used
  values first, then the most common catalog entries.
- **Decision — size**: the backend returns at most 20 suggestions; the list
  shows 8 rows before it scrolls (contracts/ui-entry.md §1). SC-001 is tested
  against the first 8.
- **Decision — source marker**: each suggestion carries `inCatalog` and
  `useCount`. The list marks catalog entries "Built-in" and record entries with
  their count ("3 in collection"); a value in both shows both (FR-016).
- **Rationale**: This is FR-012 made exact, with the two gaps it leaves closed:
  a single typed word matching a later word (so "wesson" is not a dead end),
  and a minimum of two characters for initialisms (so "r" does not match every
  make whose initialism starts with r, which US2-1 rules out: "r" finds Ruger
  only by prefix).

## 5. Where suggestions are computed: the backend, per keystroke

- **Decision**: A read command, `suggest_entries`, computes the list on each
  keystroke: it queries the distinct values of the field with their counts and
  lowest id, merges the catalog, matches and ranks (§4), and returns the top
  20. The frontend drops any response older than the latest request.
  Nothing is cached between calls: not in the backend, not in the frontend.
- **Rationale**: FR-011 and the recorded decision ("merged at query time … the
  suggestion sources keep no copy of their own"): a value from a deleted
  firearm is gone from the very next keystroke, with no cache to invalidate.
  The matching logic lives once, next to the catalog and the import that also
  needs it. Tauri IPC costs about a millisecond; the queries are
  `SELECT make, COUNT(*), MIN(id) FROM firearms GROUP BY make` (and likewise
  caliber and cartridge), each covered by a single-column index, and for
  model `GROUP BY make, model`, a scan of at most 10,000 rows.
  `tests/performance_test.rs` gains a case at 10,000 firearms with 10,000
  distinct models (the worst case) and holds the command to 50 ms, half of
  SC-004's 100 ms, leaving the rest for IPC and rendering.
- **Alternatives considered**: Loading the whole vocabulary into the form when
  it opens and matching in TypeScript (rejected: a second implementation of §3
  and §4 that import would not share, and a copy of the user's values held in
  the webview for as long as the form is open, against the recorded decision);
  a debounce (rejected: not needed at this cost, and it would delay the list
  the user is waiting on; stale responses are dropped instead); an index on
  `(make, model)` (not added: the performance test decides, and adds it only
  if the scan misses the budget).

## 6. Snapping (FR-013, FR-014)

- **Decision — the target** for a value `v` entered in field `f`:
  1. The catalog spelling for `f` whose key equals `entry_key(v)` (cartridge
     names for cartridge, class spellings for caliber). Aliases are never
     targets (FR-013: they only narrow the list).
  2. Otherwise, among the values of `f` on record with that key, the one used
     by the most firearms; on a tie, the earliest recorded (lowest id).
  3. Otherwise `v` itself, trimmed.
  Models snap against models of every make: a model spelling is the same
  spelling whichever make it is recorded with.
- **Decision — the command**: `settle_entry(field, text)` returns the settled
  value and whether and why it changed (`catalog` or `record`). For the
  cartridge field it also returns the caliber the settled cartridge derives
  (§7), itself snapped by this same rule against the caliber field, so the
  form needs one round trip when the user leaves the cartridge field.
- **Decision — when the form settles** (details in contracts/ui-entry.md §2):
  when the user leaves one of the four fields or picks a suggestion, but only if
  the field's text differs from what it was last settled to, and, when editing
  a saved firearm, from the saved value (FR-014: an untouched field, or one
  typed back to its saved value, is never snapped). A settle response is
  applied only if the field still holds the text that was sent. Saving first
  waits for any settle in flight and settles the focused field; if that changes
  a value, the form shows the change and **does not save**: the user sees it
  and saves again ("visibly, before saving", FR-013). This only happens when
  the user presses Enter inside a field without leaving it.
- **Decision — the backend does not snap on create or update.** Snapping is
  guidance applied where the user can see it: in the form, and on import with a
  report. `create_firearm` and `update_firearm` store what they are given,
  after trimming and the entry rules (§9).
- **Rationale**: One rule in one function (`services::suggestions::snap`),
  called by `settle_entry` and by import. Snapping silently in
  `create_firearm` would change a value the user did not see changed.

## 7. Caliber derivation and the guess (FR-003, FR-005, FR-007)

- **Decision**: `derive_caliber(cartridge) -> Option<(caliber, source)>` in
  `services/cartridges`:
  1. **Catalog**: if `entry_key(cartridge)` equals the key of a catalog name
     *or alias*, the result is that entry's class, source `catalog`. An alias
     names exactly one entry (§2 invariant 2), so "9x19" or ".300 BLK" typed
     and kept as typed still gets its caliber from the catalog.
  2. **Guess**: otherwise read the **leading designation** from the start of
     the name, after NFKC, lower case, `×`→`x`, and a decimal comma between
     digits read as a point (`7,62x39`). The first rule that matches wins:

     | # | Pattern at the start | Designation | Examples |
     |---|---|---|---|
     | 1 | 1–2 digits, optional space or `-`, then `ga`, `ga.` or `gauge` | `N gauge` | "16 gauge", "12ga 3in" |
     | 2 | optional `.`, 3 digits, optional space or `-`, then `bore` | `.NNN bore` | "410 bore", ".410 Bore" |
     | 3 | a number (1–2 digits, optional `.` and 1–2 digits), optional space, `mm` | `Nmm` | "9mm", "7mm-08 Ackley", "6.5mm Wildcat" |
     | 4 | the same number, optional spaces, `x`, optional spaces, a digit | `Nmm` | "6.5x47 Wildcat", "7,62x39", "9 x 19" |
     | 5 | one digit, `.`, 1–2 digits, then a space, `-` or end | `N.Nmm` | "6.5 Creedmoor Improved", "5.56 Custom" |
     | 6 | `.` then 2–3 digits, then a non-digit or end | `.NN` / `.NNN` | ".30 Custom Improved", ".308 Ackley", ".45-70 Wildcat" |
     | 7 | 2–3 digits (no dot), then a space, `-`, `/` or end | `.NN` / `.NNN`, **only if step 3 finds it** | "22 Hornet Improved", "45-70 Wildcat", "30-06 AI" |

     No match, no guess ("Wildcat Special", "Hornet"). Rule 7's restriction
     keeps a bare number that is not a known bore ("16 Special") from becoming
     a made-up ".16".
  3. **Map the designation to its class** (source `guess`):
     a. if its key is a class's key, that class ("9mm" → "9mm", ".30" →
        ".30", "12 gauge" → "12 gauge");
     b. else if it is the leading designation of catalog names or aliases,
        the class of the best-ranked of them (".308" → ".30", ".380" →
        "9mm", "7.62mm" → ".30", "5.56mm" → ".22", ".38" → ".357", ".410"
        → ".410 bore");
     c. else, for rules 1–6, the designation itself (".19 Calhoon" →
        ".19", "14 gauge" → "14 gauge"): it was read, not invented;
     d. else (rule 7, unknown) no guess.
- **FR-007 corpus**: `src-tauri/tests/fixtures/caliber_guess_corpus.tsv`, one
  line per name with the expected caliber and source (or `none`), covering
  rimfire, centerfire pistol and rifle, inch and metric, gauges and bores,
  aliases, cross-notation classes, comma decimals and unguessable names. At
  least: .22 LR, .22 Long Rifle, .17 HMR, .300 BLK, .308 Improved, .30 Custom
  Improved, 7.62x39, 7,62x39, 6.5x47 Wildcat, 6.5 Creedmoor Ackley, 9x19,
  9mm, 9mm Luger, .380 Custom, .45 ACP, 45-70 Wildcat, 12 gauge, 12ga, 16
  gauge, .410 bore, 410, Wildcat Special, Hornet, 16 Special, and "" (blank).
  `tests/caliber_guess_test.rs` reads it (SC-002). The expected results are
  in the fixture, not computed by the code under test.
- **Rationale**: FR-005's three notations become explicit rules with examples,
  and the mapping step is what makes a custom cartridge land in the same group
  as its catalog relatives (the spec's clarification). "No guess rather than a
  wrong one" (SC-002) is why rule 7 is restricted and why nothing after the
  leading designation is read.
- **Alternatives considered**: Reading a designation anywhere in the name
  ("Remington .223") (rejected: FR-005 reads the start, and a number later in
  a name is often a case length or a year); rounding an unknown inch
  designation to the nearest class (".318" → ".32") (rejected: that is
  inventing a value; ".318" is shown as a guess the user can correct).

## 8. The caliber field's state on the form (FR-003, FR-006)

- **Decision**: The form keeps a small state beside the caliber text, in a
  pure module (`caliberDerivation.ts`) with its own unit tests:
  - `mode: "derived" | "edited"`: a new firearm starts `derived`; editing a
    saved firearm starts `edited` (its saved caliber counts as already edited,
    FR-006). Typing in the caliber field makes it `edited`. Leaving the caliber
    field empty makes it `derived` again, and then, if a cartridge is entered,
    fills it from that cartridge at once.
  - `source: "catalog" | "guess" | null`: how a derived caliber was filled; a
    guess is marked on the field until the user edits the caliber or leaves it
    as it is and saves.
  - `suggestion: string | null`: in `edited` mode, the caliber the current
    cartridge derives when it differs from the field's caliber, offered with a
    one-action "Use …" button (US1-7). Picking it sets the caliber and keeps
    `edited` mode.
  When the cartridge is settled (§6): in `derived` mode the caliber field takes
  the derived caliber (or is emptied, with the prompt of US1-4, when there is
  none); in `edited` mode only `suggestion` changes.
- **Resumed drafts**: `mode` and `source` are part of the form state kept in a
  pending-changes draft, so `FORM_VERSION` goes from 1 to 2 (older drafts are
  discarded, as 003 research §16 intends).
- **Rationale**: The derived/edited rule is the part of FR-006 most likely to
  regress, and as a pure reducer it can be tested exhaustively without
  rendering the form.

## 9. Entry rules for make, model, cartridge and caliber (FR-015)

- **Decision**: One function, `check_entry_text(field, value)`, in
  `services/entry_text.rs`: after trimming, at most 100 characters (Unicode
  scalar values, `chars().count()`) and no control characters (Unicode `Cc`);
  make, model and caliber must also be non-empty. `FirearmInput::normalized()`
  now trims make, model and caliber (it trimmed only the optional text fields
  before) and turns a blank cartridge into `None`. The frontend mirrors the
  length and control-character check for an immediate message
  (`Array.from(text).length`), and the backend stays the authority.
- **Decision — existing values over the cap** (spec Assumptions): `create_firearm`
  checks all four fields; `update_firearm` checks a field only when its trimmed
  value differs from the stored one, so a record whose old make is 120
  characters can still be edited elsewhere; import checks every value in the
  sheet (FR-026: a violation is a row error).
- **Rationale**: 001 left make and model untrimmed (002 research §3's note on
  whitespace), and the identity comparison compensated with `lower(trim())`.
  FR-015 trims them, which also makes the stored values match the identity
  trigger's plain column comparison.

## 10. Action types: a seeded lookup, a type mapping, and a backstop (FR-017 to FR-021)

- **Decision — tables**: `action_types (id, name UNIQUE, sort_order UNIQUE)`
  and `firearm_type_actions (firearm_type_id, action_type_id)` with a composite
  primary key, both created in `0001_initial.sql` and seeded in
  `0003_seed_firearm_types.sql` with fixed ids, so an id means the same action
  in every build. `firearms.action_type_id` is a nullable foreign key. The
  seed is FR-018's table exactly, in its order; "Other" gets no rows.
- **Decision — the migration file keeps its name.** `schema_migrations` stores
  migration names and treats an unknown one as a newer layout, so renaming
  0003 would make every existing development database fail to open as
  "newer" rather than as incompatible. Its header comment says it seeds both
  lookup lists. No new migration is added (CLAUDE.md: edit in place while
  unreleased).
- **Decision — the rule** "an action is allowed for a type when the type has
  no mapping rows, or has a row for that action" is enforced three times, like
  002's identity rule: the command layer (`check_action_allowed`, a readable
  field error on `actionTypeId` for create, update and import), and a
  `BEFORE INSERT` / `BEFORE UPDATE` trigger pair on `firearms` that
  `RAISE(ABORT)`s otherwise. SC-008 says a firearm can **never** be saved with
  a disallowed action, so the backstop covers any path that bypasses the
  command layer; a test proves it with raw SQL.
- **Decision — how the frontend gets the list**: a read command,
  `list_action_types`, returns the actions in order and, per firearm type, the
  allowed action ids (an empty list meaning "all"). `CollectionProvider` loads
  it once per open database, beside the policies.
- **Decision — backup tracking and the seed coverage test**: both tables get
  the three backup-due triggers, like `firearm_types`, because
  `backup_due_tracking_test` requires every table to track changes or be
  housekeeping (they never change at run time, so the triggers never fire).
  `human_seed_coverage_test`'s `is_user_table` gains both names beside
  `firearm_types`: seeded lookups, not user data. `firearms.action_type_id`
  and `firearms.cartridge` are user columns and must be seeded.
- **Rationale**: The recorded decision models action type exactly like
  `FirearmType`, with a join table for the mapping, and both the mapping and
  the names then have one source (the seed) that the frontend reads rather
  than copies.
- **Alternatives considered**: A text enum with a `CHECK` (like `condition`)
  and a mapping in code (rejected: the decision on the issue asks for the
  lookup table, and #12 will extend the mapping for new types, which a join
  table takes without a code change to the rule); copying the list and
  mapping into TypeScript like `FIREARM_TYPE_OPTIONS` (rejected: two copies of
  the mapping with nothing to keep them in step).

## 11. Grouping, search and display (FR-008, FR-020, FR-027)

- **Decision — grouping**: `GroupBy` gains `cartridge` and `action_type`.
  Firearms with none go in a group labelled **"Unspecified"**, which sorts
  last. Cartridge groups are otherwise alphabetical; action groups follow the
  action list's order (`sort_order`), like origin's fixed order (002 research
  §10).
- **Decision — one term, "Unspecified"** (spec Clarifications): constitution
  III requires one term across screens, and 002 used "Not specified" for
  origin. This feature renames every use of it: origin's group key and fixed
  order in `list_firearms` (`["Domestic", "Imported", "Re-imported",
  "Unspecified"]`), the origin choice card in `ORIGIN_OPTIONS`, and
  `originLabel(null)` on the record page. The Action select's empty option is
  "Unspecified" too. The tests that assert the old label
  (`list_firearms_test.rs`, `FirearmForm.test.tsx`,
  `FirearmRecordPage.test.tsx`) change with it, and 002's documents get an
  "amended by 004" pointer.
- **Decision — search**: `firearms_fts` gains `cartridge` and
  `action_type_name` (the action's name, looked up in the FTS triggers the same
  way `firearm_type_name` is). Action names are fixed at run time, so the index
  never goes stale. The existing prefix-phrase query already finds "7.62x39"
  in "7.62x39mm" and "bolt" in "Bolt action"; `fts_search_test.rs` pins both.
- **Decision — display**: `FirearmSummary` gains `cartridge` and
  `actionTypeName`. The list's caliber cell and the tile show
  `cartridge (caliber)` or the caliber alone; the list gains an **Action**
  column after **Type**. The caliber cell is hidden when grouped by caliber or
  cartridge, and the action column when grouped by action type.
- **Performance**: grouping is done in Rust over rows the list query already
  returns, joined to `action_types` by primary key; `performance_test.rs`
  adds grouping by cartridge and action and a search on each at 10,000 rows
  against the existing 1 s and 500 ms budgets.

## 12. Import: columns by header, snapping, derived calibers (FR-022 to FR-026)

- **Finding**: `spreadsheet::row_from_cells` reads cells **by position**.
  002's columns were added just before the export-only `photo_filenames`, so
  older sheets still lined up. FR-022 puts the new columns after `caliber`,
  which would shift every later column of a sheet exported before this
  feature, breaking FR-023 and US4-8.
- **Decision — read by header**: import maps each column by its header name
  (trimmed, any letter case) to the field of the same name in `COLUMNS`.
  Columns it does not know are ignored, as `photo_filenames` already is; a
  known column that is missing reads as blank in every row, so a missing
  `cartridge` or `action_type` column means none (FR-023) and a missing
  `make` column gives the usual per-row "Missing required field" errors. A
  header row that names a column twice is a file-level `VALIDATION_ERROR`,
  since which of the two to read would be a guess. Export keeps writing
  `COLUMNS` in order, so a round trip is unchanged.
- **Decision — the order of work per import**:
  1. Read every row. Snapshot the values on record for the four fields
     (FR-026: "at the start of the import").
  2. **Sheet pass**: for each field, group the sheet's non-blank cells by
     `entry_key`. For keys with no catalog spelling and no value on record,
     pick the spelling used by the most rows, the earliest row on a tie.
  3. For each row: check the four cells with `check_entry_text` (a violation
     is a row error); snap each by §6 against the snapshot, falling back to
     the sheet pass's spelling; if `caliber` is blank and `cartridge` is
     given, derive it (§7) and snap it too, or fail the row with "caliber:
     Caliber is required; it couldn't be worked out from the cartridge
     "Wildcat Special"." (FR-025); resolve `action_type` (below); then validate,
     match and create exactly as today. Matching (FR-026) therefore compares
     the snapped values.
  Snapping against the start-of-import snapshot, rather than the collection as
  rows are added, makes the result independent of row order apart from the
  explicit earliest-row tie-break.
- **Decision — `action_type` cells**: matched against `action_types.name`
  ignoring letter case and surrounding whitespace. Unknown: "action_type:
  unknown action type "flintlockish"". Not allowed for the row's type:
  "action_type: Pump action doesn't apply to a Handgun." Both are row errors
  (FR-024), like an unknown `firearm_type`.
- **Decision — the report**: `ImportResult` gains `derivedCalibers` (row,
  cartridge, caliber, `catalog` or `guess`) and `snappedValues` (row, field,
  value in the sheet, value recorded). Both list rows that were imported or
  became conflicts; a failed row records nothing, so it lists nothing. The
  count of values changed by snapping is `snappedValues.length` (SC-007).
  A conflict's pending input is already snapped, so `resolve_import_conflicts`
  needs no change: an overwrite or duplicate saves the snapped values the
  report already listed.
- **Alternatives considered**: Appending the new columns at the end instead
  (rejected: FR-022 places them after `caliber`, where a person reading the
  sheet expects them, and positional reading would stay fragile for the next
  column); snapping against the live collection as rows are imported
  (rejected above).

## 13. The suggestion list component: a shared, hand-built combobox (FR-016)

- **Decision**: A new shared primitive, `src/components/Combobox.tsx`,
  following the WAI-ARIA Authoring Practices "combobox with listbox popup"
  pattern (list autocomplete, no automatic selection): the `input` has
  `role="combobox"`, `aria-expanded`, `aria-controls` and
  `aria-activedescendant`; the popup is a `role="listbox"` of `role="option"`
  rows. Focus never leaves the input. The popup renders through Radix
  `Popover` (already a dependency, used by `DateField`) anchored to the input
  with its open auto-focus prevented, so it is not clipped by the form
  dialog's scroll container. It is exported from `components/index.ts`, and a
  feature-level `EntryField` in `features/firearms/` connects it to
  `suggest_entries` and `settle_entry`.
- **Rationale**: Radix has no combobox, and `Select` cannot take free text.
  One shared component keeps the four fields identical (constitution III). A
  hand-built component of this size is less to review than a new dependency's
  data-collection and license checks (constitution, Security & Data Handling
  and Licensing), and the app already builds its own `DateField` the same way.
- **Real input**: WebKitGTK focus and relayout bugs are invisible to
  WebDriver (DEVELOPMENT.md, "Real keyboard and mouse input"). The E2E spec
  drives the list with real key events through that harness for the
  keyboard-only scenario (US2-10).
- **Alternatives considered**: `downshift` or `react-aria` (rejected: a new
  runtime dependency for one component); a native `<datalist>` (rejected: it
  cannot show the source marker, its ranking is the browser's, and WebKitGTK's
  rendering of it is inconsistent).

## 14. Deleting a firearm leaves nothing behind (FR-011, SC-005)

- **Decision**: No new storage holds a cartridge, make, model or caliber
  outside the `firearms` row and its FTS entry: suggestions are computed per
  call (§5), the catalog is in the binary (§1), and settled values exist only
  in the form until saved. Deleting a firearm already removes its row and FTS
  entry and reclaims the freed pages (`secure_delete`, `reclaim_freed_space`).
  `deletion_wipe_test.rs` extends its "the text is gone from the file's bytes"
  check to a unique custom cartridge, and a test asserts it is gone from
  `suggest_entries` right after the delete (US2-7) and still present after a
  disposal.
- **Pending drafts**: a draft kept at a lock holds the form's unsaved text
  (003 FR-039) in `pending_changes`, which suggestions never read, so an
  unsaved value is not suggested (US2-8).
