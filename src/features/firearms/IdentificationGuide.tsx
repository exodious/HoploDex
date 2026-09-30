import { useLayoutEffect, useRef } from "react";
import { Dialog, placeFocus } from "../../components";
import "./forms.css";

/** Which part the guide opens at: the top (Where it came from), or Part 2
 * with its heading scrolled into view and focused. */
export type GuidePart = "origin" | "registration";

export interface IdentificationGuideProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Defaults to the top. */
  part?: GuidePart;
}

interface Example {
  title: string;
  /** The text after the first label: "What you see" for a stamp on the
   * firearm, "What you have" for paperwork. */
  whatYouSee: string;
  howToRecordIt: string;
}

/** specs/002-firearm-identification contracts/ui-identification.md §8: the
 * six worked examples required by FR-015/SC-007. Static reference text with
 * no data behind it — it never blocks, rejects or second-guesses a choice
 * of origin, and states the FR-006 disclaimer once at the top. */
const EXAMPLES: Example[] = [
  {
    title: "Re-imported M1 Carbine",
    whatYouSee:
      "A U.S.-made carbine that was exported, later brought back, and now bears an importer's stamp.",
    howToRecordIt:
      "Origin: Re-imported. Main make, model and serial number are the U.S. maker's. Importer is the name in the stamp. Country of manufacture is not asked for — it's always the United States for a re-imported firearm.",
  },
  {
    title: "Importer adopted the maker's marks",
    whatYouSee: "A pistol made in Austria and stamped with its U.S. importer's name.",
    howToRecordIt:
      "Origin: Imported. Country of manufacture: Austria. Main model and serial number are the maker's. Importer is the name in the stamp. No original maker's marks are needed and no importer location is recorded.",
  },
  {
    title: "Importer assigned its own serial number",
    whatYouSee:
      "The paperwork shows an importer-assigned serial number, different from the maker's.",
    howToRecordIt:
      "Main make, model and serial number are the importer's, per the paperwork. The maker's original make, model and serial number go in Original maker's marks.",
  },
  {
    title: "Older surplus import with only the maker's marks",
    whatYouSee: "A surplus firearm stamped only with its original maker's marks.",
    howToRecordIt: "Origin: Imported. Main marks are the maker's. Importer is optional.",
  },
  {
    title: "Same serial from two wartime makers",
    whatYouSee:
      'Two firearms of the same wartime model share a serial number stamped by different makers, one with an added suffix such as an "X".',
    howToRecordIt:
      'Record the actual manufacturer as the make (for example "Inland", not the government nomenclature) and any suffix exactly as stamped, as part of the serial number. The make then tells the records apart.',
  },
  {
    title: "Pre-1968 domestic firearms whose maker restarted numbering",
    whatYouSee:
      "Two domestic firearms from the same maker share a make, model and serial number because the maker restarted its numbering.",
    howToRecordIt:
      "Enter a year of manufacture on each firearm. Two firearms can share make, model and serial number when each has a year and the years differ.",
  },
];

/** specs/005-regulated-item-types research.md §12: the two registered-item
 * examples, in the same pairs. Paperwork is what the owner has, so the first
 * label is "What you have". Like the origin examples they are illustrations,
 * not rules. */
const REGISTRATION_EXAMPLES: Example[] = [
  {
    title: "Suppressor bought on a Form 4, registered to a trust",
    whatYouSee:
      "A suppressor, and the approved Form 4 that came back for it, showing a trust as the registrant.",
    howToRecordIt:
      'Type: Suppressor. Caliber rating: the largest bore it is rated for, for example ".30". Registered as: Suppressor. Form: "Form 4". Approved: the date on the approved form. Registered to: the trust\'s name as it appears on the form. Attach the approved form as a document.',
  },
  {
    title: "Rifle made into a short-barreled rifle on a Form 1",
    whatYouSee: "A rifle, and the approved Form 1 for making it a short-barreled rifle.",
    howToRecordIt:
      'Type stays Rifle. Make and serial number are as marked on the firearm. Registered as: Short-barreled rifle. Form: "Form 1". Approved: the date on the approved form. Registered to: the owner. If several uppers are used on the receiver, list them in Notes.',
  },
];

function ExampleList({
  label,
  examples,
  have,
}: {
  label: string;
  examples: Example[];
  have: string;
}) {
  return (
    <ol className="hd-origin-guide__list" aria-label={label}>
      {examples.map((example) => (
        <li key={example.title} className="hd-origin-guide__example">
          <h4 className="hd-origin-guide__example-title">{example.title}</h4>
          <p>
            <strong>{have}:</strong> {example.whatYouSee}
          </p>
          <p>
            <strong>How to record it:</strong> {example.howToRecordIt}
          </p>
        </li>
      ))}
    </ol>
  );
}

/** Part 2's heading. Opened from the Registration section it is scrolled into
 * view and focused before the dialog places focus itself. */
function PartHeading({ focus, children }: { focus: boolean; children: string }) {
  const ref = useRef<HTMLHeadingElement>(null);
  useLayoutEffect(() => {
    if (!focus) return;
    ref.current?.scrollIntoView?.({ block: "start" });
    placeFocus(ref.current);
  }, [focus]);
  return (
    <h3 ref={ref} tabIndex={-1} className="hd-form-section__title hd-origin-guide__heading">
      {children}
    </h3>
  );
}

/** The "How do I record this?" guide (FR-015, SC-007; specs/005-regulated-item-types
 * FR-014): a shared `Dialog` opened from beside the origin control or from the
 * Registration section. Static text with worked examples, introduced as
 * examples so no one reads Austria or Inland as a rule; nothing in it
 * depends on the collection. */
export function IdentificationGuide({
  open,
  onOpenChange,
  part = "origin",
}: IdentificationGuideProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="How to record where a firearm came from and how it's registered"
      description="Worked examples, each showing what you might have and how to record it. Yours may not match any of them exactly: use the closest as a guide."
      size="lg"
    >
      <p className="hd-origin-guide__disclaimer">
        Record what is stamped on the firearm and what your paperwork says. HoploDex doesn't check
        it against any rules or decide what is regulated.
      </p>
      <PartHeading focus={false}>Where it came from</PartHeading>
      <ExampleList label="Where it came from" examples={EXAMPLES} have="What you see" />
      <div className="hd-origin-guide__part">
        <PartHeading focus={part === "registration"}>Registered items</PartHeading>
        <ExampleList
          label="Registered items"
          examples={REGISTRATION_EXAMPLES}
          have="What you have"
        />
      </div>
    </Dialog>
  );
}
