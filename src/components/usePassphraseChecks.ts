import { useCallback, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import { listen } from "../services/tauriClient";
import { CLEAR_PASSPHRASE_FIELDS } from "./PassphraseField";
import type { PassphraseFieldHandle } from "./PassphraseField";

/** The fewest characters a new passphrase may have (FR-003). */
export const MIN_PASSPHRASE_CHARS = 12;

/** How long typing pauses before a problem is shown. A problem put right
 * clears at once. */
export const PASSPHRASE_CHECK_DELAY_MS = 400;

export const PASSPHRASE_TOO_SHORT = "Use at least 12 characters.";
export const PASSPHRASES_DIFFER = "The passphrases don't match.";
export const PASSPHRASE_UNCHANGED = "Choose a passphrase different from the current one.";

export interface PassphraseCheckFields {
  /** The passphrase being set. */
  passphrase: RefObject<PassphraseFieldHandle | null>;
  /** Its confirmation. */
  confirmation: RefObject<PassphraseFieldHandle | null>;
  /** The current passphrase, when it is being changed: it must be typed,
   * and the new one must differ from it. */
  current?: RefObject<PassphraseFieldHandle | null>;
}

export interface PassphraseCheckErrors {
  passphrase?: string;
  confirmation?: string;
}

export interface PassphraseChecks {
  /** Every field is filled in, the new passphrase is long enough and
   * differs from the current one, and the confirmation matches. */
  ready: boolean;
  errors: PassphraseCheckErrors;
  /** For each field's `onInput`. */
  check: () => void;
  /** Forgets every result, once the fields have been emptied. */
  clear: () => void;
}

function length(text: string): number {
  return [...text.normalize("NFC")].length;
}

/** What is wrong, from the fields as they are now. The passphrases are read
 * here and dropped when it returns: only the verdict leaves (FR-007). */
function inspect(fields: PassphraseCheckFields): { ready: boolean; errors: PassphraseCheckErrors } {
  const current = fields.current?.current?.read().normalize("NFC");
  const passphrase = (fields.passphrase.current?.read() ?? "").normalize("NFC");
  const confirmation = (fields.confirmation.current?.read() ?? "").normalize("NFC");

  const errors: PassphraseCheckErrors = {};
  if (passphrase && current && passphrase === current) {
    errors.passphrase = PASSPHRASE_UNCHANGED;
  } else if (confirmation && length(passphrase) < MIN_PASSPHRASE_CHARS) {
    // Only once the confirmation is being typed: until then the strength
    // hint is advice enough, and a half-typed passphrase isn't an error.
    errors.passphrase = PASSPHRASE_TOO_SHORT;
  }
  // Any difference, a confirmation a character short included: the pause
  // before it shows is what lets the typing finish.
  if (confirmation && confirmation !== passphrase) {
    errors.confirmation = PASSPHRASES_DIFFER;
  }
  const ready =
    current !== "" &&
    length(passphrase) >= MIN_PASSPHRASE_CHARS &&
    passphrase !== current &&
    confirmation === passphrase;
  return { ready, errors };
}

function sameErrors(a: PassphraseCheckErrors, b: PassphraseCheckErrors): boolean {
  return a.passphrase === b.passphrase && a.confirmation === b.confirmation;
}

/** Checks a new passphrase and its confirmation as they are typed
 * (contracts/ui-databases.md §0), for the forms that set one. `ready` follows
 * every keystroke, so the form's button enables the moment all is well;
 * problems wait for a pause in typing. The fields stay uncontrolled: each
 * check reads them afresh, and the timer that waits for the pause holds no
 * value, only the fields' handles. */
export function usePassphraseChecks(fields: PassphraseCheckFields): PassphraseChecks {
  const [ready, setReady] = useState(false);
  const [errors, setErrors] = useState<PassphraseCheckErrors>({});
  const shown = useRef<PassphraseCheckErrors>({});
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const fieldsRef = useRef(fields);
  fieldsRef.current = fields;

  const show = useCallback((next: PassphraseCheckErrors) => {
    if (sameErrors(shown.current, next)) return;
    shown.current = next;
    setErrors(next);
  }, []);

  const check = useCallback(() => {
    const now = inspect(fieldsRef.current);
    setReady(now.ready);
    // Put right: cleared at once. Anything new waits for the pause.
    show({
      passphrase:
        now.errors.passphrase === shown.current.passphrase ? now.errors.passphrase : undefined,
      confirmation:
        now.errors.confirmation === shown.current.confirmation
          ? now.errors.confirmation
          : undefined,
    });
    clearTimeout(timer.current);
    timer.current = setTimeout(
      () => show(inspect(fieldsRef.current).errors),
      PASSPHRASE_CHECK_DELAY_MS,
    );
  }, [show]);

  const clear = useCallback(() => {
    clearTimeout(timer.current);
    setReady(false);
    show({});
  }, [show]);

  // The fields empty themselves when the screen locks or the computer
  // sleeps; so does what was found in them. After theirs, whichever of the
  // listeners runs first.
  useEffect(
    () =>
      listen<unknown>(CLEAR_PASSPHRASE_FIELDS, () => {
        clear();
        setTimeout(() => setReady(inspect(fieldsRef.current).ready), 0);
      }),
    [clear],
  );

  useEffect(() => () => clearTimeout(timer.current), []);

  return { ready, errors, check, clear };
}
