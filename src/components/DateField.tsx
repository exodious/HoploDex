import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import * as Popover from "@radix-ui/react-popover";
import { parseDateInput, toIso, todayIso } from "../lib/dates";
import { Icon } from "./Icon";
import { TextField } from "./TextField";
import type { TextFieldProps } from "./TextField";
import "./components.css";

export interface DateFieldProps extends Omit<
  TextFieldProps,
  "value" | "onChange" | "type" | "trailing"
> {
  /** The raw text being edited; parse it with `parseDateInput` on submit. */
  value: string;
  onValueChange: (text: string) => void;
}

/**
 * Date input that accepts typed dates (2024-03-14 or 3/14/2024) plus a
 * calendar popover that closes as soon as a day is picked. Replaces the
 * native `<input type="date">`, whose WebKitGTK picker stayed open after
 * choosing a date (spec_TODO) and whose segment-typing garbled pasted ISO
 * dates.
 */
export function DateField({ value, onValueChange, onBlur, label, ...props }: DateFieldProps) {
  const [open, setOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const parsed = parseDateInput(value);
  const selected = parsed.ok ? parsed.iso : null;

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <TextField
        {...props}
        ref={inputRef}
        label={label}
        placeholder={props.placeholder ?? "YYYY-MM-DD"}
        autoComplete="off"
        value={value}
        onChange={(e) => onValueChange(e.target.value)}
        onBlur={(e) => {
          if (parsed.ok && parsed.iso) onValueChange(parsed.iso);
          onBlur?.(e);
        }}
        trailing={
          <Popover.Trigger asChild>
            <button
              type="button"
              className="hd-input__action"
              aria-label={`Choose ${label.toLowerCase()} from a calendar`}
              disabled={props.disabled}
            >
              <Icon name="calendar" />
            </button>
          </Popover.Trigger>
        }
      />
      <Popover.Portal>
        <Popover.Content
          className="hd-popover"
          align="end"
          sideOffset={6}
          collisionPadding={12}
          onOpenAutoFocus={(e) => e.preventDefault()}
          onCloseAutoFocus={(e) => {
            e.preventDefault();
            inputRef.current?.focus();
          }}
        >
          <Calendar
            selected={selected}
            onSelect={(iso) => {
              onValueChange(iso);
              setOpen(false);
            }}
          />
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

const MONTHS = Array.from({ length: 12 }, (_, i) =>
  new Intl.DateTimeFormat(undefined, { month: "long", timeZone: "UTC" }).format(
    new Date(Date.UTC(2000, i, 1)),
  ),
);
const WEEKDAYS = Array.from({ length: 7 }, (_, i) =>
  new Intl.DateTimeFormat(undefined, { weekday: "narrow", timeZone: "UTC" }).format(
    new Date(Date.UTC(2023, 0, 1 + i)), // 2023-01-01 was a Sunday
  ),
);
const FULL_DATE = new Intl.DateTimeFormat(undefined, { dateStyle: "full", timeZone: "UTC" });

function parts(iso: string): [number, number, number] {
  const [y, m, d] = iso.split("-").map(Number);
  return [y, m, d];
}

function shift(iso: string, days: number): string {
  const [y, m, d] = parts(iso);
  const date = new Date(Date.UTC(y, m - 1, d + days));
  return toIso(date.getUTCFullYear(), date.getUTCMonth() + 1, date.getUTCDate());
}

function shiftMonths(iso: string, months: number): string {
  const [y, m, d] = parts(iso);
  const target = new Date(Date.UTC(y, m - 1 + months, 1));
  const lastDay = new Date(
    Date.UTC(target.getUTCFullYear(), target.getUTCMonth() + 1, 0),
  ).getUTCDate();
  return toIso(target.getUTCFullYear(), target.getUTCMonth() + 1, Math.min(d, lastDay));
}

function Calendar({
  selected,
  onSelect,
}: {
  selected: string | null;
  onSelect: (iso: string) => void;
}) {
  const today = todayIso();
  const [cursor, setCursor] = useState(selected ?? today);
  const gridRef = useRef<HTMLTableElement>(null);
  // Focus follows the cursor day on open and during arrow-key movement
  // (roving tabindex), but not when the month/year controls move it —
  // those keep their own focus.
  const focusCursor = useRef(true);
  const [year, month] = parts(cursor);

  useEffect(() => {
    if (!focusCursor.current) return;
    gridRef.current?.querySelector<HTMLButtonElement>(`[data-iso="${cursor}"]`)?.focus();
  }, [cursor]);

  function moveFromHeader(iso: string) {
    focusCursor.current = false;
    setCursor(iso);
  }

  const leadingBlanks = new Date(Date.UTC(year, month - 1, 1)).getUTCDay();
  const daysInMonth = new Date(Date.UTC(year, month, 0)).getUTCDate();
  const cells: (string | null)[] = [
    ...Array<null>(leadingBlanks).fill(null),
    ...Array.from({ length: daysInMonth }, (_, i) => toIso(year, month, i + 1)),
  ];
  const weeks: (string | null)[][] = [];
  for (let i = 0; i < cells.length; i += 7) weeks.push(cells.slice(i, i + 7));

  const thisYear = Number(today.slice(0, 4));
  const years = Array.from({ length: thisYear + 11 - 1850 }, (_, i) => thisYear + 10 - i);

  function handleKeyDown(event: KeyboardEvent) {
    const moves: Record<string, () => string> = {
      ArrowLeft: () => shift(cursor, -1),
      ArrowRight: () => shift(cursor, 1),
      ArrowUp: () => shift(cursor, -7),
      ArrowDown: () => shift(cursor, 7),
      PageUp: () => shiftMonths(cursor, event.shiftKey ? -12 : -1),
      PageDown: () => shiftMonths(cursor, event.shiftKey ? 12 : 1),
      Home: () => shift(cursor, -new Date(Date.UTC(year, month - 1, parts(cursor)[2])).getUTCDay()),
      End: () =>
        shift(cursor, 6 - new Date(Date.UTC(year, month - 1, parts(cursor)[2])).getUTCDay()),
    };
    const move = moves[event.key];
    if (move) {
      event.preventDefault();
      focusCursor.current = true;
      setCursor(move());
    }
  }

  return (
    <div className="hd-calendar">
      <div className="hd-calendar__head">
        <button
          type="button"
          className="hd-calendar__nav"
          aria-label="Previous month"
          onClick={() => moveFromHeader(shiftMonths(cursor, -1))}
        >
          <Icon name="chevronLeft" size={16} />
        </button>
        <select
          className="hd-calendar__select"
          aria-label="Month"
          value={month}
          onChange={(e) => moveFromHeader(shiftMonths(cursor, Number(e.target.value) - month))}
        >
          {MONTHS.map((name, i) => (
            <option key={name} value={i + 1}>
              {name}
            </option>
          ))}
        </select>
        <select
          className="hd-calendar__select"
          aria-label="Year"
          value={year}
          onChange={(e) =>
            moveFromHeader(shiftMonths(cursor, (Number(e.target.value) - year) * 12))
          }
        >
          {years.map((y) => (
            <option key={y} value={y}>
              {y}
            </option>
          ))}
        </select>
        <button
          type="button"
          className="hd-calendar__nav"
          aria-label="Next month"
          onClick={() => moveFromHeader(shiftMonths(cursor, 1))}
        >
          <Icon name="chevronRight" size={16} />
        </button>
      </div>

      <table className="hd-calendar__grid" ref={gridRef} onKeyDown={handleKeyDown}>
        <thead>
          <tr>
            {WEEKDAYS.map((day, i) => (
              <th key={i} scope="col" abbr={day}>
                {day}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {weeks.map((week, w) => (
            <tr key={w}>
              {week.map((iso, d) => (
                <td key={d}>
                  {iso && (
                    <button
                      type="button"
                      className="hd-calendar__day"
                      data-iso={iso}
                      tabIndex={iso === cursor ? 0 : -1}
                      aria-label={FULL_DATE.format(new Date(`${iso}T00:00:00Z`))}
                      aria-pressed={iso === selected}
                      aria-current={iso === today ? "date" : undefined}
                      onClick={() => onSelect(iso)}
                    >
                      {parts(iso)[2]}
                    </button>
                  )}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>

      <div className="hd-calendar__foot">
        <button type="button" className="hd-link" onClick={() => onSelect(today)}>
          Today
        </button>
        {selected && (
          <button type="button" className="hd-link" onClick={() => onSelect("")}>
            Clear date
          </button>
        )}
      </div>
    </div>
  );
}
