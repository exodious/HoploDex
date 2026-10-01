import { FirearmName } from "../app/FirearmName";
import { useNavigation } from "../app/navigation";
import type { Route } from "../app/navigation";
import { accessoryNameText } from "./recordNames";
import type { RecordLabel } from "./types";

// specs/006-accessory-links FR-005, contracts/ui-accessories.md (rules): a
// record of either kind is named by `RecordName`. A firearm is named as
// `FirearmName` names it (make, model and “nickname”, 001 FR-031), with its
// type after a dot where asked. An accessory is "{make} {model} · {kind}":
// one of make and model alone as it is, the kind alone when both are blank.

/** The pages a link can be followed from (the route's `from`). */
const FROM_PAGES = ["accessories", "collection", "insurance", "firearm", "accessory"] as const;

export interface RecordNameProps {
  label: RecordLabel;
  /** Renders the name as a link to the record, through `navigation.open`. */
  link?: boolean;
  /** Adds the firearm's type after a dot; an accessory always shows its kind. */
  withType?: boolean;
}

function NameText({ label, withType }: { label: RecordLabel; withType: boolean }) {
  if (label.record.kind === "firearm") {
    return (
      <>
        <FirearmName
          firearm={{ make: label.make ?? "", model: label.model ?? "", nickname: label.nickname }}
        />
        {withType && ` · ${label.typeName}`}
      </>
    );
  }
  return <>{accessoryNameText(label.make, label.model, label.typeName)}</>;
}

export function RecordName({ label, link = false, withType = false }: RecordNameProps) {
  const navigation = useNavigation();
  const text = <NameText label={label} withType={withType} />;
  if (!link) return text;

  const page: string = navigation.route.page;
  const from = FROM_PAGES.find((candidate) => candidate === page) ?? "collection";
  return (
    <button
      type="button"
      className="hd-link"
      onClick={() =>
        // A firearm route's `from` is still the two pages it was first
        // reached from; widening it is the Mounted section's (US2).
        navigation.open({ page: label.record.kind, id: label.record.id, from } as Route)
      }
    >
      {text}
    </button>
  );
}
