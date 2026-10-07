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
