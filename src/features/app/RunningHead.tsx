import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { Icon } from "../../components";
import { BackLink } from "./BackLink";
import { useNavigation } from "./navigation";
import "./RunningHead.css";

export interface RunningHeadProps {
  /** The page's own heading block. The strip shows once it has scrolled
   * up under the top bar. */
  anchor: HTMLElement | null;
  /** The page's heading, focused when the name is used to return to the top
   * (it needs tabIndex={-1}). */
  headingId: string;
  /** The record's name, in plain text. */
  title: string;
  /** A short identifier set beside the name: a serial or policy number. */
  stamp?: string | null;
  /** The page's actions, at the small size. */
  actions: ReactNode;
}

/** A slim strip under the top bar for a long record page, the way a drawing
 * set repeats a reduced title block on every continuation sheet. Once the
 * page's heading scrolls away it keeps the way back, the record's name
 * (which returns to the top) and the record's actions within reach (FR-041). */
export function RunningHead({ anchor, headingId, title, stamp, actions }: RunningHeadProps) {
  const { back } = useNavigation();
  const shown = useScrolledPast(anchor);
  if (!shown) return null;

  function toTop() {
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    document.getElementById(headingId)?.focus({ preventScroll: true });
    window.scrollTo({ top: 0, behavior: reduce ? "auto" : "smooth" });
  }

  return (
    <div className="hd-runhead">
      <div className="hd-runhead__inner">
        {back && <BackLink target={back} />}
        <button type="button" className="hd-runhead__title" onClick={toTop} title="Back to top">
          <span className="hd-runhead__name">{title}</span>
          {stamp && <span className="hd-serial hd-runhead__stamp">{stamp}</span>}
          <Icon name="up" size={16} className="hd-runhead__up" />
          <span className="hd-sr-only">, back to top</span>
        </button>
        <div className="hd-runhead__actions">{actions}</div>
      </div>
    </div>
  );
}

/** Whether `anchor` has scrolled up past the bottom of the top bar. False
 * until it exists, and where IntersectionObserver doesn't. */
function useScrolledPast(anchor: HTMLElement | null): boolean {
  const [past, setPast] = useState(false);

  useEffect(() => {
    setPast(false);
    if (!anchor || typeof IntersectionObserver === "undefined") return;
    const topbar =
      parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--topbar-height")) ||
      56;
    const observer = new IntersectionObserver(
      ([entry]) => {
        const edge = entry.rootBounds?.top ?? topbar;
        // Out of view and above the edge: scrolled past, not yet reached.
        setPast(!entry.isIntersecting && entry.boundingClientRect.bottom <= edge);
      },
      { rootMargin: `-${topbar}px 0px 0px 0px` },
    );
    observer.observe(anchor);
    return () => observer.disconnect();
  }, [anchor]);

  return past;
}
