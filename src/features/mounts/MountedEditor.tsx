import { useId } from "react";
import { Button, ChangedMark, placeFocus } from "../../components";
import { MountChooser } from "./MountChooser";
import { recordKey } from "./recordKey";
import { recordNameText, recordNameWithType } from "./recordNames";
import type { RecordLabel, RecordRef } from "./types";
import type { MountedChanges } from "./mountedChanges";
import "./mounts.css";

// 014 contracts/ui.md §5 (014 FR-024 to FR-026): the Mounted list of an active
// firearm's or accessory's edit form. Adding and removing only change the form;
// each change is marked, says what Save will do, and has its own revert. The
// form saves them with the record (014 FR-027).

export interface MountedEditorProps {
  /** The record whose form this is. */
  record: RecordRef;
  /** What is mounted directly on it, as saved. */
  saved: RecordLabel[];
  changes: MountedChanges;
  onChange: (changes: MountedChanges) => void;
  /** The form's Mounted on now, for the chooser (014 FR-026). */
  mountedOn: RecordLabel | null;
  /** The id of the add field, so a form can open at it. */
  addId?: string;
  /** A save refused on the list (`fieldErrors.mounted`), shown at the add field. */
  error?: string;
}

export function MountedEditor({
  record,
  saved,
  changes,
  onChange,
  mountedOn,
  addId,
  error,
}: MountedEditorProps) {
  const ownId = useId();
  const inputId = addId ?? `${ownId}-add`;
  const removed = new Set(changes.removes);
  const listed = new Set([
    ...saved.map((label) => recordKey(label.record)),
    ...changes.adds.map((add) => recordKey(add.label.record)),
  ]);
  const focusAdd = () => document.getElementById(inputId);

  function add(label: RecordLabel | null, from?: RecordLabel | null) {
    if (!label) return;
    const key = recordKey(label.record);
    // A saved entry added back is its removal undone: no change (014 FR-025).
    if (removed.has(key))
      onChange({ ...changes, removes: changes.removes.filter((k) => k !== key) });
    else if (!listed.has(key))
      onChange({ ...changes, adds: [...changes.adds, { label, from: from ?? null }] });
  }

  function remove(label: RecordLabel) {
    const key = recordKey(label.record);
    onChange({ ...changes, removes: [...changes.removes, key] });
    // The row stays, its Remove gone: focus goes to the add field.
    setTimeout(() => placeFocus(focusAdd()), 0);
  }

  return (
    <div className="hd-field hd-mounted-editor" data-field="mounted">
      <span className="hd-field__label" id={`${ownId}-label`}>
        Mounted
      </span>
      {saved.length + changes.adds.length > 0 ? (
        <ul className="hd-mounted" aria-labelledby={`${ownId}-label`}>
          {saved.map((label) => {
            const key = recordKey(label.record);
            const isRemoved = removed.has(key);
            const noteId = `${ownId}-${key}-note`;
            return (
              <li key={key} className="hd-mounted__entry hd-mounted__entry--direct">
                <div className="hd-mounted__line">
                  <span
                    className="hd-mounted__name"
                    aria-describedby={isRemoved ? noteId : undefined}
                  >
                    {recordNameWithType(label)}
                  </span>
                  {isRemoved ? (
                    <ChangedMark
                      label={recordNameText(label)}
                      change={{
                        saved: "Mounted here",
                        onRevert: () =>
                          onChange({
                            ...changes,
                            removes: changes.removes.filter((k) => k !== key),
                          }),
                      }}
                      id={`${ownId}-${key}-changed`}
                      focusTarget={focusAdd}
                    />
                  ) : (
                    <span className="hd-mounted__actions">
                      <Button
                        size="sm"
                        variant="ghost"
                        aria-label={`Remove ${recordNameText(label)}`}
                        onClick={() => remove(label)}
                      >
                        Remove
                      </Button>
                    </span>
                  )}
                </div>
                {isRemoved && (
                  <p className="hd-mounted__on" id={noteId}>
                    Will be unmounted.
                  </p>
                )}
              </li>
            );
          })}
          {changes.adds.map(({ label, from }) => {
            const key = recordKey(label.record);
            const noteId = `${ownId}-${key}-note`;
            return (
              <li key={key} className="hd-mounted__entry hd-mounted__entry--direct">
                <div className="hd-mounted__line">
                  <span className="hd-mounted__name" aria-describedby={noteId}>
                    {recordNameWithType(label)}
                  </span>
                  <ChangedMark
                    label={recordNameText(label)}
                    change={{
                      saved: "Not mounted here",
                      onRevert: () =>
                        onChange({
                          ...changes,
                          adds: changes.adds.filter((a) => recordKey(a.label.record) !== key),
                        }),
                    }}
                    id={`${ownId}-${key}-changed`}
                    focusTarget={focusAdd}
                  />
                </div>
                <p className="hd-mounted__on" id={noteId}>
                  {from
                    ? `Will be mounted here, moving from ${recordNameText(from)} with everything mounted on it.`
                    : "Will be mounted here, with everything mounted on it."}
                </p>
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="hd-field__hint">Nothing is mounted on it.</p>
      )}
      <MountChooser
        id={inputId}
        role="item"
        record={record}
        label="Add to Mounted"
        placeholder="Search for an accessory or firearm"
        value={null}
        onChange={add}
        hostMountedOn={{ on: mountedOn?.record ?? null }}
        exclude={listed}
        error={error}
        hint="Nothing changes until you save."
      />
    </div>
  );
}
