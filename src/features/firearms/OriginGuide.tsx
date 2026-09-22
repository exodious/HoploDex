import { Dialog } from "../../components";
import "./forms.css";

export interface OriginGuideProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

interface Example {
  title: string;
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

/** The "How do I record this?" guide (FR-015, SC-007): a shared `Dialog`
 * opened from beside the origin control. Static text with worked examples;
 * nothing in it depends on the collection. */
export function OriginGuide({ open, onOpenChange }: OriginGuideProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="How to record where a firearm came from"
      size="lg"
    >
      <p className="hd-origin-guide__disclaimer">
        Record what is stamped on the firearm and what your paperwork says. The app does not check
        it against any rules.
      </p>
      <ol className="hd-origin-guide__list">
        {EXAMPLES.map((example) => (
          <li key={example.title} className="hd-origin-guide__example">
            <h3 className="hd-origin-guide__example-title">{example.title}</h3>
            <p>
              <strong>What you see:</strong> {example.whatYouSee}
            </p>
            <p>
              <strong>How to record it:</strong> {example.howToRecordIt}
            </p>
          </li>
        ))}
      </ol>
    </Dialog>
  );
}
