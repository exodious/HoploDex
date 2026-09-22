import { useId } from "react";
import type { ReactNode } from "react";
import { Icon } from "./Icon";
import "./components.css";

export interface DisclosureProps {
  title: string;
  /** One line under the title saying what the group holds. While closed it
   * should read back what is recorded, so closing never hides a value. */
  summary?: ReactNode;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Heading level of the title, to sit correctly under its section. */
  headingLevel?: 3 | 4;
  children: ReactNode;
}

/** A group of optional fields a form can fold away (WAI-ARIA disclosure:
 * a heading holding a button with `aria-expanded`). Controlled, so the form
 * decides when it opens: on a recorded value, or on an error inside it.
 * Closed, its fields are not rendered, so they are never focusable. */
export function Disclosure({
  title,
  summary,
  open,
  onOpenChange,
  headingLevel = 4,
  children,
}: DisclosureProps) {
  const id = useId();
  const panelId = `${id}-panel`;
  const Heading = `h${headingLevel}` as const;
  return (
    <div className="hd-disclosure" data-open={open || undefined}>
      <Heading className="hd-disclosure__heading">
        <button
          type="button"
          className="hd-disclosure__trigger"
          aria-expanded={open}
          aria-controls={panelId}
          onClick={() => onOpenChange(!open)}
        >
          <Icon name="chevronRight" className="hd-disclosure__chevron" />
          <span className="hd-disclosure__text">
            <span className="hd-disclosure__title">{title}</span>
            {summary && <span className="hd-disclosure__summary">{summary}</span>}
          </span>
        </button>
      </Heading>
      {open && (
        <div id={panelId} className="hd-disclosure__panel">
          {children}
        </div>
      )}
    </div>
  );
}
