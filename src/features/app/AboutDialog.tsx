import { useEffect, useRef, useState } from "react";
import { Dialog, Disclosure, placeFocus, TextField } from "../../components";
import { filterComponents, loadNotices } from "./aboutNotices";
import type { ComponentNotice, NoticesDocument } from "./aboutNotices";
import "../app/AppShell.css";
import "./about.css";

/** How long the notices load before "Loading the license notices…" is shown,
 * so the usual load, in milliseconds, doesn't flash it: the closing screen's
 * rule (003 SC-005; `ClosingScreen`'s `SHOW_AFTER_MS`). */
const SHOW_AFTER_MS = 1000;

const LOAD_FAILED =
  "The license notices couldn't be loaded. They are also in the NOTICES.txt file that came with HoploDex.";

export interface AboutDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type Load =
  { state: "loading" } | { state: "ready"; notices: NoticesDocument } | { state: "failed" };

/** About HoploDex (010 contracts/ui.md §2, §3): the program block, the
 * license and the notices of what ships. The document is read when the
 * dialog opens and dropped when it closes; nothing here calls a command
 * (010 SC-005). */
export function AboutDialog({ open, onOpenChange }: AboutDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange} title="About HoploDex" size="lg">
      <AboutContent />
    </Dialog>
  );
}

/** The program block compiled in with the build (010 data-model.md, "Program
 * block"). */
function ProgramBlock({ permission }: { permission: string | null }) {
  const program = __HOPLODEX_PROGRAM__;
  return (
    <div className="hd-about__program">
      <div className="hd-about__name">
        <h3 className="hd-about__title">{program.name}</h3>
        <span className="hd-about__version">Version {program.version}</span>
      </div>
      <p>
        Copyright © {program.copyrightYears} {program.copyrightHolder}
      </p>
      <p>
        HoploDex is free software: you can share it and change it under the terms of the GNU General
        Public License, version 3 only, as published by the Free Software Foundation.
      </p>
      <p>It comes with no warranty, to the extent permitted by law. See the license for details.</p>
      {permission !== null && <AdditionalPermission text={permission} />}
      <p>
        Source code: <span className="hd-about__source">{program.sourceUrl}</span>
      </p>
    </div>
  );
}

/** The additional permission: its heading line as a run-in, then its
 * paragraphs, which `licenses/additional-permission.txt` wraps by hand. */
function AdditionalPermission({ text }: { text: string }) {
  const [heading, ...rest] = text.trim().split("\n");
  const paragraphs = rest
    .join("\n")
    .split(/\n\s*\n/)
    .map((paragraph) => paragraph.replace(/\s*\n\s*/g, " ").trim())
    .filter(Boolean);
  return (
    <div className="hd-about__permission">
      <p>
        <strong>{heading.trim()}</strong>
        {paragraphs.length > 0 && ` ${paragraphs[0]}`}
      </p>
      {paragraphs.slice(1).map((paragraph, index) => (
        <p key={index}>{paragraph}</p>
      ))}
    </div>
  );
}

/** A license text, word for word: focusable so the keyboard can scroll it. */
function TextBlock({ name, text }: { name: string; text: string }) {
  return (
    <pre className="hd-about__text" role="region" aria-label={name} tabIndex={0}>
      {text}
    </pre>
  );
}

function summaryOf(component: ComponentNotice): string {
  const { declared, license, version } = component;
  return declared && declared !== license
    ? `${version} · ${declared}, used under ${license}`
    : `${version} · ${license}`;
}

function countOf(shown: number, total: number): string {
  const noun = total === 1 ? "component" : "components";
  return shown === total ? `${total} ${noun}` : `${shown} of ${total} ${noun}`;
}

function AboutContent() {
  const [load, setLoad] = useState<Load>({ state: "loading" });
  const [late, setLate] = useState(false);
  const [filter, setFilter] = useState("");
  const [licenseOpen, setLicenseOpen] = useState(false);
  const [openRows, setOpenRows] = useState<ReadonlySet<string>>(new Set());
  const body = useRef<HTMLDivElement>(null);
  const filterField = useRef<HTMLInputElement>(null);
  const filterValue = useRef("");
  filterValue.current = filter;

  useEffect(() => {
    const controller = new AbortController();
    loadNotices(controller.signal).then(
      (notices) => {
        if (!controller.signal.aborted) setLoad({ state: "ready", notices });
      },
      () => {
        if (!controller.signal.aborted) setLoad({ state: "failed" });
      },
    );
    return () => controller.abort();
  }, []);

  const loading = load.state === "loading";
  useEffect(() => {
    if (!loading) return;
    const timer = setTimeout(() => setLate(true), SHOW_AFTER_MS);
    return () => clearTimeout(timer);
  }, [loading]);

  // The license disclosure doesn't exist until the notices have loaded, so the
  // dialog's own first-control rule finds nothing to focus at open. Focus goes
  // to it as it appears, unless the reader has already moved into the body
  // (010 contracts/ui.md §3).
  const ready = load.state === "ready";
  useEffect(() => {
    if (!ready || !body.current) return;
    const active = document.activeElement;
    if (active && body.current.contains(active)) return;
    placeFocus(body.current.querySelector<HTMLElement>(".hd-about__license button"));
  }, [ready]);

  // Escape clears a filter that has text before it closes the dialog. The
  // dialog's own Escape listens on the document in the capture phase, so this
  // goes in earlier, on the window, and cancels the key.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.target !== filterField.current) return;
      if (!filterValue.current) return;
      event.preventDefault();
      setFilter("");
    }
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, []);

  function toggleRow(key: string, next: boolean) {
    setOpenRows((current) => {
      const rows = new Set(current);
      if (next) rows.add(key);
      else rows.delete(key);
      return rows;
    });
  }

  const notices = load.state === "ready" ? load.notices : null;
  const shown = notices ? filterComponents(notices.components, filter) : [];

  return (
    <div className="hd-about" ref={body}>
      <ProgramBlock permission={notices ? notices.additionalPermission : null} />

      {load.state === "loading" && late && (
        <p className="hd-about__status">Loading the license notices…</p>
      )}
      {load.state === "failed" && (
        <p className="hd-banner hd-banner--error" role="alert">
          {LOAD_FAILED}
        </p>
      )}

      {notices && (
        <>
          <div className="hd-about__license">
            <Disclosure
              title="GNU General Public License, version 3"
              summary="The full text of the license HoploDex is under."
              headingLevel={3}
              open={licenseOpen}
              onOpenChange={setLicenseOpen}
            >
              <TextBlock name="GNU General Public License text" text={notices.gpl} />
            </Disclosure>
          </div>

          <div className="hd-about__components">
            <h3 className="hd-about__heading">Built with</h3>
            <p>
              HoploDex includes these components by other authors. Each is used under the license
              shown, whose text follows its name.
            </p>
            <TextField
              label="Find a component"
              type="search"
              ref={filterField}
              value={filter}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setFilter(event.target.value)}
            />
            <p className="hd-about__count" aria-live="polite">
              {countOf(shown.length, notices.components.length)}
            </p>
            {shown.length === 0 ? (
              <p>No component matches “{filter.trim()}”.</p>
            ) : (
              <div className="hd-about__list">
                {shown.map((component) => {
                  const key = `${component.name}\u0000${component.version}`;
                  return (
                    <Disclosure
                      key={key}
                      title={component.name}
                      summary={summaryOf(component)}
                      headingLevel={4}
                      open={openRows.has(key)}
                      onOpenChange={(next) => toggleRow(key, next)}
                    >
                      {component.carrier && (
                        <p className="hd-about__carrier">
                          Built into {component.carrier.name} {component.carrier.version}
                        </p>
                      )}
                      {component.texts.map((id, index) => (
                        <TextBlock
                          key={`${id}.${index}`}
                          name={`${component.name} license text`}
                          text={notices.texts[id]}
                        />
                      ))}
                    </Disclosure>
                  );
                })}
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
