// specs/006-accessory-links FR-005: record names as plain text.

/** An accessory's make and model, joined by a space; blank text is none. */
function accessoryMakeModel(make: string | null, model: string | null): string {
  return [make, model]
    .map((part) => part?.trim() ?? "")
    .filter((part) => part !== "")
    .join(" ");
}

/** An accessory's name as plain text (FR-005), for places that take a string,
 * such as a form's unsaved-changes label: "{make} {model} · {kind}". */
export function accessoryNameText(
  make: string | null,
  model: string | null,
  kindName: string,
): string {
  const makeModel = accessoryMakeModel(make, model);
  return makeModel ? `${makeModel} · ${kindName}` : kindName;
}
