/** Loads `notices.json` and shapes it for the About HoploDex dialog (010
 * data-model.md, "Notices document"; 010 contracts/notices.md). */

/** The one `notices.json` layout this build reads. Any other value shows as
 * the load failure, not as a wrong screen (010 data-model.md). */
export const NOTICES_FORMAT = 1;

/** Where the build puts the document: the app's own origin, no command and
 * no other address (010 SC-005). */
const NOTICES_URL = "/notices.json";

export interface ProgramBlock {
  name: string;
  version: string;
  copyrightYears: string;
  copyrightHolder: string;
  sourceUrl: string;
}

export interface ComponentCarrier {
  kind: string;
  name: string;
  version: string;
}

export interface ComponentNotice {
  name: string;
  version: string;
  kind: string;
  /** The license HoploDex uses the component under. */
  license: string;
  /** The expression the package declares, present only for a choice. */
  declared?: string;
  /** Ids into the document's `texts`, in display order. */
  texts: string[];
  source: string;
  /** For a hand-recorded component, the shipped package that carries it. */
  carrier?: ComponentCarrier;
}

export interface NoticesDocument {
  format: typeof NOTICES_FORMAT;
  program: ProgramBlock;
  target: string;
  gpl: string;
  additionalPermission: string;
  components: ComponentNotice[];
  texts: Record<string, string>;
  development?: boolean;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isString(value: unknown): value is string {
  return typeof value === "string";
}

function isComponent(value: unknown, texts: Record<string, string>): value is ComponentNotice {
  if (!isRecord(value)) return false;
  const { name, version, kind, license, declared, source, carrier } = value;
  if (![name, version, kind, license, source].every(isString) || !name) return false;
  if (declared !== undefined && !isString(declared)) return false;
  if (
    carrier !== undefined &&
    !(
      isRecord(carrier) &&
      isString(carrier.kind) &&
      isString(carrier.name) &&
      isString(carrier.version)
    )
  ) {
    return false;
  }
  const ids = value.texts;
  return Array.isArray(ids) && ids.length > 0 && ids.every((id) => isString(id) && id in texts);
}

/** Checks the parsed document's shape, so a malformed file is one failure. */
function isNoticesDocument(value: unknown): value is NoticesDocument {
  if (!isRecord(value) || value.format !== NOTICES_FORMAT) return false;
  const { program, target, gpl, additionalPermission, components, texts } = value;
  if (!isRecord(program) || !isString(target) || !isString(gpl)) return false;
  if (!isString(additionalPermission) || !isRecord(texts) || !Array.isArray(components)) {
    return false;
  }
  if (!Object.values(texts).every(isString)) return false;
  return components.every((component) => isComponent(component, texts as Record<string, string>));
}

/** Fetches the build's notices. Rejects on a failed request, a response that
 * isn't OK, a body that isn't JSON, a `format` other than 1 or a malformed
 * shape; the dialog shows each the same way. */
export async function loadNotices(signal?: AbortSignal): Promise<NoticesDocument> {
  const response = await fetch(NOTICES_URL, { signal });
  if (!response.ok) throw new Error(`notices.json: HTTP ${response.status}`);
  const document: unknown = await response.json();
  if (!isNoticesDocument(document)) throw new Error("notices.json: unsupported or malformed");
  return document;
}

/** The components whose name holds `query`, ignoring case, in their order. An
 * empty query keeps them all. */
export function filterComponents<T extends { name: string }>(components: T[], query: string): T[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return components;
  return components.filter((component) => component.name.toLowerCase().includes(needle));
}
