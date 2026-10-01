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

/** Any record's name as plain text (FR-005): a firearm "{make} {model}
 * “{nickname}”" (001 FR-031), an accessory "{make} {model} · {kind}". */
export function recordNameText(label: {
  record: { kind: "firearm" | "accessory" };
  make: string | null;
  model: string | null;
  nickname: string | null;
  typeName: string;
}): string {
  if (label.record.kind === "accessory") {
    return accessoryNameText(label.make, label.model, label.typeName);
  }
  const name = `${label.make ?? ""} ${label.model ?? ""}`.trim();
  return label.nickname ? `${name} “${label.nickname}”` : name;
}

/** {@link recordNameText}, with a firearm's type after a dot as an accessory's
 * kind already is (FR-005): the name lists show a record by. */
export function recordNameWithType(label: Parameters<typeof recordNameText>[0]): string {
  const name = recordNameText(label);
  return label.record.kind === "firearm" ? `${name} · ${label.typeName}` : name;
}
