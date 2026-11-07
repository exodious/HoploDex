# Screenshots

_Part of the [development docs](../../DEVELOPMENT.md)._

For pull requests that change the UI, take screenshots of the real app
(WebKitGTK, the engine users get), not of a browser:

```bash
npm run build
npm run screenshots                            # -> e2e/screenshots-out/ (git-ignored)
npm run screenshots -- --screenshots=/tmp/pr   # somewhere else
```

This runs `e2e/screenshots/screens.e2e.ts` through the E2E harness, under Xvfb
at a fixed 1200×800 window (for Windows, see [Windows](windows.md)). It starts
at the database chooser listing the
[human-testing databases](human-testing.md), seeded into the session's
throwaway sandbox, shoots it and the create dialog, unlocks "Main collection"
with the seed's passphrase, and walks the main screens and dialogs
(collection list and tiles, a full record, the record scrolled so its pinned
strip shows, the edit/coverage/dispose dialogs, the unsaved-changes question,
add firearm, insurance, a policy, import and export), then the database menu's
dialogs (settings, change passphrase, restore from a backup, the guide), the
pending-changes question after a lock, the closing screen with its backup
bar, and "Shared collection" refused as open on another computer. Then `e2e/screenshots/first-run.e2e.ts`, in an
unseeded sandbox, shoots the first-run chooser and a new database's
disk-encryption note. Every screen is taken in light and dark mode, writing
`<nn>-<screen>-<theme>.png`. Long pages and dialogs are captured whole. The
names don't change between runs, so for before/after pairs, run it on the base
branch and then on yours:

```bash
git switch main && npm run build && npm run screenshots -- --screenshots=e2e/screenshots-out/before
git switch my-branch && npm run build && npm run screenshots -- --screenshots=e2e/screenshots-out/after
```

When a change adds a screen, add it to the walk. For a one-off shot inside
any other spec, call `shot("name")` from `e2e/support/screenshots.ts`. It
saves only when the run was started with `--screenshots`, so a normal
`test:e2e` is unaffected:

```bash
npm run test:e2e -- --screenshots --spec e2e/specs/record-firearm.e2e.ts
```

The seed writes into a new `seed` folder in the sandbox, and the harness
moves the app's config directory there so its recent list names the seeded
databases. Specs read the seed's passphrase from
`HOPLODEX_E2E_SEED_PASSPHRASE`, and the sandbox's documents folder from
`HOPLODEX_E2E_DOCUMENTS`.

## Screenshots in a pull request

GitHub has no place on a pull request to keep images, so each UI pull request
keeps its screenshots on an orphan branch, `evidence/<name>-screenshots`
(`<name>` is the feature's folder, such as `011-flatpak`, or the fix's branch
name, such as `fix-114-116-117-122`): one commit with no parent, holding only
the images, flat at the top, named `before-<shot>.png` and `after-<shot>.png`.

1. Take the before shots on `main` and the after shots on your branch, as
   above, into `e2e/screenshots-out/before` and `e2e/screenshots-out/after`
   (git-ignored, and inside the checkout, so the dev container's shots reach
   the host). Keep only the shots the change affects.
2. Commit them without switching the working tree, so it works from any
   checkout or worktree, then push the commit as the branch. It stops before
   the push when there are no shots, and the `[]` it prints after the commit's
   hash shows it has no parent:

   ```bash
   name=my-feature
   tree=$(find e2e/screenshots-out/before e2e/screenshots-out/after -name '*.png' |
     while read -r f; do
       printf '100644 blob %s\t%s\n' "$(git hash-object -w "$f")" "$(basename "$(dirname "$f")")-$(basename "$f")"
     done | git mktree)
   test -n "$(git ls-tree "$tree")" &&
     commit=$(git commit-tree "$tree" -m "Screenshots for $name") &&
     git log --format='%H [%P]' -1 "$commit" &&
     git push origin "${commit}:refs/heads/evidence/${name}-screenshots"
   ```

   To replace the shots, rerun the snippet and push with `--force`.

3. In the pull request's description, a `## Screenshots` section says where
   and how the shots were taken (what before and after are, and the branch
   they are on) and shows them in before | after tables, each image linked as
   `https://github.com/exodious/HoploDex/raw/evidence/<name>-screenshots/<file>.png`:

   ```markdown
   | Before | After |
   |---|---|
   | ![The guide, before](https://github.com/exodious/HoploDex/raw/evidence/my-feature-screenshots/before-07-guide-light.png) | ![The guide, after](https://github.com/exodious/HoploDex/raw/evidence/my-feature-screenshots/after-07-guide-light.png) |
   ```

The evidence branches are kept after the merge, so the links keep working.

## The Flatpak's store shots

The five images the Flatpak's AppStream metadata points at
(`flatpak/screenshots/`: `chooser`, `collection`, `record`, `document-preview`
and `accessories`) are taken by their own command, not by the walk above, and
**only when someone retakes them on purpose** (to show a new feature, say):
a build, `npm test`, `npm run screenshots` and `npm run test:e2e` never touch
them (`011 FR-017`; `011 research.md` §13).

```bash
scripts/dev-container.sh --store-shots bash -c 'npm run build && npm run screenshots:store'
```

It runs `e2e/screenshots/store.e2e.ts` on Linux in the dev container, in the
walk's setting (Xvfb, 1200×800, light theme), and only when the run passes
replaces the five files in `flatpak/screenshots/` together; the run itself
writes to `e2e/screenshots-out/store/`. Run twice on the same build, it gives
byte-identical files. To make that so:

- The seed is made in `/home/alex/Documents`, so the chooser shows
  `/home/alex/Documents/HoploDex`, a path that reads like a user's own. That
  folder is a tmpfs `scripts/dev-container.sh --store-shots` mounts for that one
  command (never the `/home/dev` volume, never the host), emptied when the run
  ends; the command refuses to run without it. The rest of the sandbox is at
  `/tmp/hoplodex-store-shots`, the same path every time, and the seed is run
  with `--fixed-times`: the backups, the
  last open and the documents' dates are stamped so the chooser reads "Today at
  10:30 AM" and the viewer "added Sep 15, 2025" whatever day it is. No text in
  view may depend on the day or time of the run; where one would, change the
  view, or give the seed a record with fixed dates, never the test.
- The seed's $99,999,999 firearm (#67, kept in the seed) has its estimated
  replacement value set to $2,500 through the app's edit form, so the total
  reads $51,300, and the collection is grouped by Type so no photo that needs a
  credit line is in view.
- `e2e/screenshots/store-shots.json` records what each shot shows, by record and
  photo with its licence, and the text the page must carry. The spec checks the
  app's view against it before every shot and fails rather than take one that
  differs; `scripts/flatpak/storeShots.test.ts` (in `npm test`) checks the file
  against `011 research.md` §13's list, the seed-photos README's licences and
  the metainfo. A retake that shows another record or photo changes that file,
  and the research note, on purpose.

Look at the five files before committing them. A different WebKitGTK, font or
Mesa in the dev container image can change every pixel, so retake all five
together, never one.
