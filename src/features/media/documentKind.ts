// The kind a document is shown as in the list and the viewer
// (specs/007-document-preview/contracts/ui-document-preview.md §1, "Kind
// labels"): the label of the recorded type from `list_document_types`, and for
// a row stored before 007 with another type, its extension in capitals.
import type { DocumentSummary, DocumentType } from "./types";

export function documentKindLabel(doc: DocumentSummary, types: DocumentType[]): string {
  const type = types.find((candidate) => candidate.mimeType === doc.mimeType);
  if (type) return type.label;
  const extension = doc.originalFilename.split(".").pop();
  return extension && extension !== doc.originalFilename ? extension.toUpperCase() : "File";
}

/** A kind label inside a sentence (ui contract §3): "Plain text" and "Spreadsheet" are
 * lower-cased, and what is written as it is keeps its case: the acronyms (PDF, TIFF, CSV,
 * RTF, an extension in capitals), the OpenDocument names and the product name Word. */
export function kindInSentence(label: string): string {
  return /^[A-Z][a-z]/.test(label) && !/^(?:OpenDocument|Word)\b/.test(label)
    ? label.charAt(0).toLowerCase() + label.slice(1)
    : label;
}
