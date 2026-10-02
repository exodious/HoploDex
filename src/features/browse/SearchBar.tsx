import { forwardRef, useId } from "react";
import { Icon } from "../../components";

export interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  busy?: boolean;
  /** The box's accessible name. */
  label?: string;
  placeholder?: string;
}

/** Free-text search across all recorded firearm information, including
 * free-form notes (FR-013, US2 Acceptance Scenarios 3-4). Escape clears. */
export const SearchBar = forwardRef<HTMLInputElement, SearchBarProps>(
  (
    {
      value,
      onChange,
      busy,
      label = "Search the collection",
      placeholder = "Search make, model, serial, caliber, notes…",
    },
    ref,
  ) => {
    const id = useId();
    return (
      <div className="hd-search">
        <label htmlFor={id} className="hd-sr-only">
          {label}
        </label>
        <div className="hd-input hd-search__box">
          <Icon name="search" className="hd-search__icon" />
          <input
            ref={ref}
            id={id}
            type="search"
            className="hd-input__control"
            placeholder={placeholder}
            autoComplete="off"
            spellCheck={false}
            value={value}
            aria-busy={busy || undefined}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape" && value) {
                e.preventDefault();
                onChange("");
              }
            }}
          />
          {value ? (
            <button
              type="button"
              className="hd-input__action"
              aria-label="Clear search"
              onClick={() => onChange("")}
            >
              <Icon name="close" size={16} />
            </button>
          ) : (
            <kbd className="hd-kbd hd-search__kbd" aria-hidden>
              /
            </kbd>
          )}
        </div>
      </div>
    );
  },
);

SearchBar.displayName = "SearchBar";
