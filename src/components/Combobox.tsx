import { useEffect, useId, useRef, useState } from "react";
import type { ChangeEvent, FocusEvent, KeyboardEvent } from "react";
import * as Popover from "@radix-ui/react-popover";
import { TextField } from "./TextField";
import type { TextFieldProps } from "./TextField";
import "./components.css";

/** One row of the list: the value, and a marker (source, count) shown at the
 * row's end in the muted style and included in its accessible name. */
export interface ComboboxOption {
  value: string;
  marker?: string;
  /** Names the row when two rows can share a value (two records with the
   * same name); `onPick` receives it in place of the value. */
  key?: string;
  /** A second muted line under the row. */
  detail?: string;
}

export interface ComboboxProps extends Omit<
  TextFieldProps,
  "value" | "onChange" | "role" | "children"
> {
  /** The text in the field. */
  value: string;
  /** The user typed: the new text. */
  onInputChange: (text: string) => void;
  /** The list for what is typed, best first. Called when the field gains
   * focus and on every keystroke; an answer for anything but the text now in
   * the field is dropped, so a slow one never replaces a fresh one. */
  loadOptions: (text: string) => Promise<ComboboxOption[]>;
  /** The user picked a row: the field should take this value (the row's
   * `key` when it has one). */
  onPick: (value: string) => void;
  /** The user left the field (Tab, a click elsewhere). Not called when a
   * row is picked. */
  onBlurSettle?: () => void;
  /** A note about an automatic change, shown in place of the hint and
   * announced politely (contracts/ui-entry.md §2). */
  note?: string;
}

/** A modal dialog's scroll lock (react-remove-scroll) cancels wheel and touch
 * moves at the document unless they start inside the dialog, and the list is
 * portaled outside it, so the list would not scroll. The browser's own
 * scrolling of the list is all that is wanted: stop these events before they
 * reach the document. */
function letListScroll(list: HTMLElement | null) {
  for (const type of ["wheel", "touchmove"]) {
    list?.addEventListener(type, (event) => event.stopPropagation(), { passive: true });
  }
}

/** How long typing must pause before the suggestion count is announced. */
export const COMBOBOX_ANNOUNCE_MS = 500;

/**
 * Text input with a list of suggestions: the WAI-ARIA "combobox with
 * listbox popup" pattern, list autocomplete with no automatic selection
 * (specs/004-cartridges-action-types research.md §13, contracts/
 * ui-entry.md §1). Focus never leaves the input. The list is a Radix
 * `Popover` anchored to the input, so a dialog's scrolling never clips it;
 * there is no "Add …" row, since typing on is how a new value is entered.
 */
export function Combobox({
  value,
  onInputChange,
  loadOptions,
  onPick,
  onBlurSettle,
  note,
  hint,
  onFocus,
  onBlur,
  onKeyDown,
  onClick,
  onMouseDown,
  ...props
}: ComboboxProps) {
  const listId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [options, setOptions] = useState<ComboboxOption[]>([]);
  const [active, setActive] = useState(-1);
  const [announcement, setAnnouncement] = useState("");
  const latestText = useRef(value);
  latestText.current = value;
  const latestRequest = useRef(0);
  // Whether the pointer is what is about to focus the field.
  const pressed = useRef(false);
  // Whether Tab was the last key pressed anywhere: it is what moves focus in
  // from another control. Focus placed by a dialog opening arrives from the
  // button that opened it too, so `relatedTarget` cannot tell the two apart.
  const tabbed = useRef(false);
  useEffect(() => {
    const note = (event: globalThis.KeyboardEvent) => {
      tabbed.current = event.key === "Tab";
    };
    document.addEventListener("keydown", note, true);
    return () => document.removeEventListener("keydown", note, true);
  }, []);
  // Popover anchors to the input's frame, which is as wide as the field.
  const anchor = useRef({
    getBoundingClientRect: () =>
      (inputRef.current?.parentElement ?? inputRef.current)?.getBoundingClientRect() ??
      new DOMRect(),
  });

  const listOpen = open && options.length > 0;

  function request(text: string) {
    const id = ++latestRequest.current;
    const apply = (next: ComboboxOption[]) => {
      if (id !== latestRequest.current || text !== latestText.current) return;
      // An empty answer to an empty list changes nothing, so nothing renders.
      setOptions((current) => (current.length === 0 && next.length === 0 ? current : next));
      setActive(-1);
    };
    loadOptions(text).then(apply, () => apply([]));
  }

  // The count is announced once typing pauses, not on every keystroke.
  useEffect(() => {
    setAnnouncement("");
    if (!listOpen) return;
    const timer = setTimeout(
      () =>
        setAnnouncement(`${options.length} ${options.length === 1 ? "suggestion" : "suggestions"}`),
      COMBOBOX_ANNOUNCE_MS,
    );
    return () => clearTimeout(timer);
  }, [listOpen, options, value]);

  useEffect(() => {
    if (active >= 0) {
      document.getElementById(`${listId}-${active}`)?.scrollIntoView?.({ block: "nearest" });
    }
  }, [active, listId]);

  function pick(picked: string) {
    setOpen(false);
    setActive(-1);
    onPick(picked);
    inputRef.current?.focus();
  }

  function openList() {
    setOpen(true);
    request(latestText.current);
  }

  function handleChange(event: ChangeEvent<HTMLInputElement>) {
    const text = event.target.value;
    latestText.current = text;
    onInputChange(text);
    setOpen(true);
    setActive(-1);
    request(text);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    onKeyDown?.(event);
    if (event.defaultPrevented) return;
    switch (event.key) {
      case "ArrowDown":
      case "ArrowUp": {
        event.preventDefault();
        if (!listOpen) {
          openList();
          break;
        }
        const count = options.length;
        setActive(
          event.key === "ArrowDown" ? (active + 1) % count : active <= 0 ? count - 1 : active - 1,
        );
        break;
      }
      case "Enter":
        if (listOpen && active >= 0) {
          // The row is picked; the form's Enter (submit) does not run.
          event.preventDefault();
          pick(options[active].key ?? options[active].value);
        } else {
          setOpen(false);
        }
        break;
      case "Escape":
        // Open: close it and keep the text. Closed: the dialog's own Escape.
        if (listOpen) {
          event.preventDefault();
          setOpen(false);
        }
        break;
      case "Tab":
        setOpen(false);
        break;
    }
  }

  function handleFocus(event: FocusEvent<HTMLInputElement>) {
    onFocus?.(event);
    // Opens for a click or a Tab from another control. A dialog placing
    // focus here as it opens is not asking for a list: Escape should close
    // the dialog, and the first key or Down brings the list up.
    if (pressed.current || tabbed.current) openList();
    pressed.current = false;
    tabbed.current = false;
  }

  function handleBlur(event: FocusEvent<HTMLInputElement>) {
    onBlur?.(event);
    setOpen(false);
    onBlurSettle?.();
  }

  return (
    <Popover.Root
      open={listOpen}
      onOpenChange={(next) => {
        if (!next) setOpen(false);
      }}
    >
      <Popover.Anchor virtualRef={anchor} />
      <TextField
        {...props}
        ref={inputRef}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={listOpen}
        aria-controls={listId}
        aria-activedescendant={listOpen && active >= 0 ? `${listId}-${active}` : undefined}
        autoComplete="off"
        value={value}
        hint={note ?? hint}
        onChange={handleChange}
        onFocus={handleFocus}
        onBlur={handleBlur}
        onKeyDown={handleKeyDown}
        onMouseDown={(event) => {
          onMouseDown?.(event);
          pressed.current = true;
        }}
        onClick={(event) => {
          onClick?.(event);
          if (!open) openList();
        }}
      />
      <span className="hd-sr-only" role="status" aria-live="polite">
        {note ?? announcement}
      </span>
      <Popover.Portal>
        <Popover.Content
          className="hd-popover hd-combobox__popover"
          // The list is the listbox below, not a dialog.
          role="presentation"
          align="start"
          sideOffset={4}
          collisionPadding={12}
          onOpenAutoFocus={(event) => event.preventDefault()}
          onCloseAutoFocus={(event) => event.preventDefault()}
          onInteractOutside={(event) => {
            // The input is outside the popover but is where the user is.
            if (event.target instanceof Node && inputRef.current?.contains(event.target)) {
              event.preventDefault();
            }
          }}
        >
          {/* A press on the list must not move focus out of the input. */}
          <ul
            id={listId}
            ref={letListScroll}
            role="listbox"
            aria-label={props.label}
            className="hd-combobox__list"
            onMouseDown={(event) => event.preventDefault()}
          >
            {options.map((option, index) => {
              const rowKey = option.key ?? option.value;
              const main = (
                <>
                  <span className="hd-combobox__value">{option.value}</span>
                  {option.marker && (
                    <>
                      {" "}
                      <span className="hd-combobox__marker">{option.marker}</span>
                    </>
                  )}
                </>
              );
              return (
                <li
                  key={rowKey}
                  id={`${listId}-${index}`}
                  role="option"
                  aria-selected={index === active}
                  className={["hd-combobox__option", option.detail && "hd-combobox__option--detail"]
                    .filter(Boolean)
                    .join(" ")}
                  onClick={() => pick(rowKey)}
                  onMouseMove={() => index !== active && setActive(index)}
                >
                  {option.detail ? (
                    <>
                      <span className="hd-combobox__row">{main}</span>
                      <span className="hd-combobox__detail">{option.detail}</span>
                    </>
                  ) : (
                    main
                  )}
                </li>
              );
            })}
          </ul>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
