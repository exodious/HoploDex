/** Moving focus to a form's field that a failed save refused. A form marks
 * each field's wrapper `data-field="<name>"`; the form's own check and the
 * backend's `fieldErrors` both name fields this way, so a refused field takes
 * the focus whichever side refused it (the constitution's UI consistency). */

/** Focuses `field`'s control inside `form`. Resolves whether it found one. */
export function focusFormField(form: ParentNode | null | undefined, field: string): boolean {
  const control = form?.querySelector<HTMLElement>(
    `[data-field="${field}"] :is(input, textarea, button)`,
  );
  control?.focus();
  return control != null;
}

/** The first of `order` the backend refused in `fieldErrors`, or `undefined`.
 * `serverName` maps a form field to the name the backend gives it, when the
 * two differ. */
export function firstRefusedField<F extends string>(
  order: readonly F[],
  fieldErrors: Record<string, string> | undefined,
  serverName?: Partial<Record<F, string>>,
): F | undefined {
  if (!fieldErrors) return undefined;
  return order.find((field) => fieldErrors[serverName?.[field] ?? field]);
}
