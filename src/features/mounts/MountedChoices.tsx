import { useId } from "react";
import { MoneyField, SegmentedControl } from "../../components";
import { MountedList } from "./MountedList";
import { recordKey } from "./recordKey";
import { recordNameWithType } from "./recordNames";
import type { MountedEntry } from "./types";
import "./mounts.css";

const CHOICES = [
  { value: "keep", label: "Keep" },
  { value: "dispose", label: "Dispose with it" },
] as const;

export interface MountedChoicesProps {
  mounted: MountedEntry[];
  /** The records disposed with it, by `recordKey`, each with the price typed
   * for it (blank: none). A record not here is kept (FR-014). */
  disposeWith: Record<string, string>;
  onChange: (next: Record<string, string>) => void;
  /** The group's heading. */
  title?: string;
  /** Whether a record disposed with it gets its own price field. The dispose
   * dialog has one; an import row has no price for it (issue #56). */
  withPrices?: boolean;
  /** Price errors by `recordKey`, once the form has been submitted. */
  errors?: Map<string, string>;
  /** Disposition-date errors by `recordKey`, once the form has been submitted. */
  dateErrors?: Map<string, string>;
  statements: string[];
}

/** The **Mounted** group (contracts/ui-accessories.md §7): everything below
 * the record, each with a Keep | Dispose with it choice and, where the form
 * has prices, an optional price for a record disposed with it. Below it,
 * what happens to the rest. Shared by the dispose dialog and the import's
 * Replace confirmation (issue #56). */
export function MountedChoices({
  mounted,
  disposeWith,
  onChange,
  title = "Mounted",
  withPrices = false,
  errors,
  dateErrors,
  statements,
}: MountedChoicesProps) {
  const titleId = useId();
  return (
    <div className="hd-dispose-mounted" role="group" aria-labelledby={titleId}>
      <h3 className="hd-form-section__title" id={titleId}>
        {title}
      </h3>
      <MountedList
        entries={mounted}
        links={false}
        detail={(entry) => {
          const key = recordKey(entry.label.record);
          const name = recordNameWithType(entry.label);
          const disposing = key in disposeWith;
          return (
            <div className="hd-mounted__choice">
              <SegmentedControl
                label={name}
                hideLabel
                size="sm"
                value={disposing ? "dispose" : "keep"}
                options={[...CHOICES]}
                onChange={(choice) => {
                  const next = { ...disposeWith };
                  if (choice === "dispose") next[key] = disposeWith[key] ?? "";
                  else delete next[key];
                  onChange(next);
                }}
              />
              {withPrices && disposing && (
                <MoneyField
                  label={`Price for ${name}`}
                  fieldClassName="hd-field--third"
                  value={disposeWith[key]}
                  onValueChange={(text) => onChange({ ...disposeWith, [key]: text })}
                  hint="Leave blank if none was received separately."
                  error={errors?.get(key)}
                />
              )}
              {disposing && dateErrors?.has(key) && (
                <p className="hd-field__error" role="alert">
                  {dateErrors.get(key)}
                </p>
              )}
            </div>
          );
        }}
      />
      {statements.map((text) => (
        <p className="hd-dispose-mounted__note" key={text}>
          {text}
        </p>
      ))}
    </div>
  );
}
