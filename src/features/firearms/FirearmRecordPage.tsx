import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { Badge, Button, ConfirmDialog, Dialog, Icon, useToast } from "../../components";
import { formatDate } from "../../lib/dates";
import { formatInches, formatWeight } from "../../lib/measure";
import { formatDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { BackLink } from "../app/BackLink";
import { firearmName, useCollection } from "../app/collectionStore";
import { FirearmName } from "../app/FirearmName";
import { useNavigation } from "../app/navigation";
import { RunningHead } from "../app/RunningHead";
import { peekResumedDraft } from "../session/usePendingDraft";
import { FirearmThumbnail } from "../browse/FirearmThumbnail";
import { CoverageDialog } from "../insurance/CoverageDialog";
import { coverageStatus, expiryLabel } from "../insurance/coverage";
import * as insuranceService from "../insurance/insuranceService";
import type { AssignCoverageInput } from "../insurance/types";
import { DocumentList } from "../media/DocumentList";
import * as mediaService from "../media/mediaService";
import { PhotoGallery } from "../media/PhotoGallery";
import { DispositionHistoryList } from "./DispositionHistoryList";
import { DisposeDialog } from "./DisposeDialog";
import { FirearmForm } from "./FirearmForm";
import type { FocusField } from "./FirearmForm";
import { RestoreDialog } from "./RestoreDialog";
import * as firearmsService from "./firearmsService";
import {
  caliberLabel,
  conditionLabel,
  dispositionLabel,
  firearmTypeOption,
  originLabel,
} from "./types";
import type {
  DisposeFirearmInput,
  Firearm,
  FirearmDetail,
  FirearmInput,
  ReverseDispositionInput,
} from "./types";
import "./record.css";

type RecordDialog = "edit" | "dispose" | "restore" | "delete" | "coverage";

export interface FirearmRecordPageProps {
  id: number;
}

function failureMessage(e: unknown, fallback: string): string {
  return e instanceof CommandFailure ? e.message : fallback;
}

/** One firearm's full record (US1, US3, US4): identity plate, coverage,
 * photos, documents, acquisition and disposition history. */
export function FirearmRecordPage({ id }: FirearmRecordPageProps) {
  const {
    firearmsById,
    policiesById,
    actionTypes,
    firearmTypes,
    registrationClasses,
    summary: valueSummary,
    revision,
    refresh,
  } = useCollection();
  const { open, back } = useNavigation();
  const notify = useToast();
  const [firearm, setFirearm] = useState<FirearmDetail | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [dialog, setDialog] = useState<RecordDialog | null>(null);
  // The field the edit form opens on, when reached from an "Add" link.
  const [editFocus, setEditFocus] = useState<FocusField>();
  // The plate, which the pinned strip waits to scroll away.
  const [plate, setPlate] = useState<HTMLElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    firearmsService
      .getFirearm(id)
      .then((loaded) => !cancelled && setFirearm(loaded))
      .catch(
        (e) => !cancelled && setLoadError(failureMessage(e, "This firearm couldn't be loaded.")),
      );
    return () => {
      cancelled = true;
    };
  }, [id, revision]);

  // Pending changes the user resumed for this firearm reopen their form
  // (FR-039), which takes them as its unsaved input.
  const loadedId = firearm?.id;
  useEffect(() => {
    const resumed = peekResumedDraft();
    if (loadedId === undefined || resumed?.kind !== "firearm" || resumed.targetId !== loadedId)
      return;
    if (resumed.mode !== "add") setDialog(resumed.mode);
  }, [loadedId]);

  if (loadError) {
    return (
      <div className="hd-record">
        {back && <BackLink target={back} />}
        <p className="hd-banner hd-banner--error" role="alert">
          <Icon name="alert" />
          <span className="hd-banner__text">{loadError}</span>
        </p>
      </div>
    );
  }
  if (!firearm) {
    return <div className="hd-record">{back && <BackLink target={back} />}</div>;
  }

  const name = firearmName(firearm);
  const summary = firearmsById.get(firearm.id);
  const policy =
    firearm.insurancePolicyId != null ? policiesById.get(firearm.insurancePolicyId) : undefined;
  const blanket = valueSummary?.blanket ?? null;
  const coverage = coverageStatus(firearm, summary?.insuranceWarning ?? "none", policy, blanket);
  // An unscheduled firearm is covered by the blanket policy in force.
  const blanketPolicy =
    !policy && blanket && firearm.status === "active"
      ? policiesById.get(blanket.policyId)
      : undefined;
  const coveringPolicy = policy ?? blanketPolicy;
  const type = firearmTypeOption(firearmTypes.types, firearm.firearmTypeId);
  // FR-027: the action's name comes from the list the collection loaded.
  const actionName = actionTypes.actions.find((a) => a.id === firearm.actionTypeId)?.name;
  const disposed = firearm.status === "disposed";

  async function afterChange(updated: Firearm, message: string) {
    // The refresh below reloads the retained history; keep what's shown
    // until it arrives.
    setFirearm((current) => ({
      ...updated,
      dispositionHistory: current?.dispositionHistory ?? [],
    }));
    setDialog(null);
    await refresh();
    notify(message);
  }

  function editField(field: FocusField) {
    setEditFocus(field);
    setDialog("edit");
  }

  async function handleUpdate(input: FirearmInput, confirmedWarnings?: boolean) {
    const updated = await firearmsService.updateFirearm(id, input, confirmedWarnings);
    await afterChange(updated, `Saved changes to ${firearmName(updated)}.`);
  }

  async function handleDispose(input: DisposeFirearmInput) {
    const updated = await firearmsService.disposeFirearm(id, input);
    await afterChange(
      updated,
      `Marked ${firearmName(updated)} as ${dispositionLabel(updated.dispositionType).toLowerCase()}.`,
    );
  }

  async function handleRestore(input: ReverseDispositionInput) {
    const restored = await firearmsService.reverseDisposition(id, input);
    await afterChange(restored, `Restored ${firearmName(restored)} to the collection.`);
  }

  async function handleCoverage(input: AssignCoverageInput) {
    const updated = await insuranceService.assignFirearmCoverage(id, input);
    await afterChange(updated, `Updated coverage for ${firearmName(updated)}.`);
  }

  async function handleDelete() {
    try {
      await firearmsService.deleteFirearm(id, true);
      await refresh();
      notify(`Deleted ${name}.`);
      back?.go();
    } catch (e) {
      notify(failureMessage(e, `${name} couldn't be deleted.`), "error");
    }
  }

  async function handleMediaChanged() {
    const updated = await firearmsService.getFirearm(id);
    setFirearm(updated);
    await refresh();
  }

  function actions(size: "md" | "sm") {
    return (
      <>
        <Button
          size={size}
          icon="pencil"
          onClick={() => {
            setEditFocus(undefined);
            setDialog("edit");
          }}
        >
          Edit
        </Button>
        {disposed ? (
          <Button size={size} icon="archive" onClick={() => setDialog("restore")}>
            Restore to collection
          </Button>
        ) : (
          <Button size={size} icon="archive" onClick={() => setDialog("dispose")}>
            Mark disposed
          </Button>
        )}
        <Button size={size} variant="ghost" icon="trash" onClick={() => setDialog("delete")}>
          Delete
        </Button>
      </>
    );
  }

  return (
    <div className="hd-record">
      <div className="hd-record__bar">
        {back && <BackLink target={back} />}
        <div className="hd-record__actions">{actions("md")}</div>
      </div>

      <article className="hd-plate" aria-labelledby="record-name" ref={setPlate}>
        <div className="hd-plate__top">
          <PlateFigure thumbnailPhotoId={firearm.thumbnailPhotoId} typeKey={type.key} />
          <header className="hd-plate__heading">
            <p className="hd-eyebrow hd-plate__type">
              {type.label}
              {disposed && (
                <Badge tone="neutral" icon="archive">
                  {dispositionLabel(firearm.dispositionType)}
                </Badge>
              )}
            </p>
            <h1 className="hd-plate__name" id="record-name" tabIndex={-1}>
              <FirearmName firearm={firearm} />
            </h1>
            <p className="hd-plate__stamp">
              <span className="hd-plate__stamp-label">Serial no.</span>
              {firearm.serialNumber ? (
                <span className="hd-serial hd-plate__serial">{firearm.serialNumber}</span>
              ) : (
                <span className="hd-plate__no-serial">None — attested as not required</span>
              )}
            </p>
          </header>
        </div>

        <dl className="hd-titleblock">
          {/* specs/004-cartridges-action-types FR-027: none recorded reads
              as an unrecorded date does. */}
          <TitleCell label="Cartridge">{firearm.cartridge ?? "—"}</TitleCell>
          <TitleCell label={caliberLabel(type.label)}>{firearm.caliber}</TitleCell>
          {/* specs/005-regulated-item-types FR-003: a type with no action
              shows no Action cell. */}
          {type.actionTypeApplies && <TitleCell label="Action">{actionName ?? "—"}</TitleCell>}
          <TitleCell label="Status">
            {disposed
              ? `${dispositionLabel(firearm.dispositionType)} ${formatDate(firearm.dispositionDate)}`
              : "Active"}
          </TitleCell>
          <TitleCell label="Replacement value">
            <span className="hd-num">{formatDollars(firearm.estimatedValue)}</span>
          </TitleCell>
          <TitleCell label="Acquired">{formatDate(firearm.acquisitionDate)}</TitleCell>
          <TitleCell label="Coverage">
            <Badge tone={coverage.tone}>{coverage.label}</Badge>
          </TitleCell>
        </dl>
      </article>

      <div className="hd-record__grid">
        <div className="hd-record__main">
          <PhotoGallery firearm={firearm} onChanged={handleMediaChanged} />

          <section className="hd-panel" aria-labelledby="identification-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="identification-title">
                Identification
              </h2>
            </header>
            <dl className="hd-facts">
              <Fact label="Origin">{originLabel(firearm.origin)}</Fact>
              {firearm.yearOfManufacture != null && (
                <Fact label="Year of manufacture">{firearm.yearOfManufacture}</Fact>
              )}
              {countryOfManufactureDisplay(firearm) && (
                <Fact label="Country of manufacture">{countryOfManufactureDisplay(firearm)}</Fact>
              )}
              {firearm.importerName && <Fact label="Importer">{firearm.importerName}</Fact>}
            </dl>
          </section>

          {hasOriginalMarks(firearm) && (
            <section className="hd-panel" aria-labelledby="original-marks-title">
              <header className="hd-panel__head">
                <h2 className="hd-panel__title" id="original-marks-title">
                  Original maker's marks
                </h2>
              </header>
              <dl className="hd-facts">
                <Fact label="Maker">{firearm.originalMake}</Fact>
                <Fact label="Model">{firearm.originalModel}</Fact>
                <Fact label="Serial number">{firearm.originalSerialNumber}</Fact>
              </dl>
            </section>
          )}

          {firearm.registrationClassId != null && (
            <section className="hd-panel" aria-labelledby="registration-title">
              <header className="hd-panel__head">
                <h2 className="hd-panel__title" id="registration-title">
                  Registration
                </h2>
                <button type="button" className="hd-link" onClick={() => editField("registration")}>
                  Edit
                </button>
              </header>
              <dl className="hd-facts">
                <Fact label="Registered as">
                  {registrationClasses.classes.find(
                    (item) => item.id === firearm.registrationClassId,
                  )?.name ?? "—"}
                </Fact>
                {firearm.registrationForm && <Fact label="Form">{firearm.registrationForm}</Fact>}
                {firearm.registrationApproved && (
                  <Fact label="Approved">{formatDate(firearm.registrationApproved)}</Fact>
                )}
                {firearm.registeredTo && <Fact label="Registered to">{firearm.registeredTo}</Fact>}
              </dl>
            </section>
          )}

          {hasPhysicalDetails(firearm) && (
            <section className="hd-panel" aria-labelledby="physical-title">
              <header className="hd-panel__head">
                <h2 className="hd-panel__title" id="physical-title">
                  Physical details
                </h2>
              </header>
              <dl className="hd-facts">
                {firearm.barrelLengthHundredths != null && (
                  <Fact label="Barrel length">{`${formatInches(firearm.barrelLengthHundredths)} in`}</Fact>
                )}
                {firearm.overallLengthHundredths != null && (
                  <Fact label="Overall length">
                    {`${formatInches(firearm.overallLengthHundredths)} in`}
                  </Fact>
                )}
                {firearm.weightTenthsOz != null && (
                  <Fact label="Weight">{formatWeight(firearm.weightTenthsOz)}</Fact>
                )}
                {firearm.capacity != null && (
                  <Fact label="Capacity">
                    {`${firearm.capacity} ${firearm.capacity === 1 ? "round" : "rounds"}`}
                  </Fact>
                )}
                {firearm.finish && <Fact label="Finish">{firearm.finish}</Fact>}
                {firearm.condition && (
                  <Fact label="Condition">{conditionLabel(firearm.condition)}</Fact>
                )}
              </dl>
            </section>
          )}

          <section className="hd-panel" aria-labelledby="notes-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="notes-title">
                Condition and notes
              </h2>
            </header>
            <TextBlock
              text={firearm.notes}
              empty="No notes recorded."
              onAdd={() => editField("notes")}
            />
            <h3 className="hd-subhead">Accessories</h3>
            <TextBlock
              text={firearm.accessories}
              empty="No accessories recorded."
              onAdd={() => editField("accessories")}
            />
          </section>

          <section className="hd-panel" aria-labelledby="history-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="history-title">
                History
              </h2>
            </header>
            <dl className="hd-facts">
              <Fact label="Acquired from">{firearm.acquisitionSource}</Fact>
              <Fact label="Date acquired">
                {firearm.acquisitionDate && formatDate(firearm.acquisitionDate)}
              </Fact>
              <Fact label="Price paid">
                {firearm.acquisitionPrice != null && formatDollars(firearm.acquisitionPrice)}
              </Fact>
              {disposed && (
                <>
                  <Fact label="Disposition">{dispositionLabel(firearm.dispositionType)}</Fact>
                  <Fact label="Transferred to">{firearm.dispositionRecipient}</Fact>
                  <Fact label="Date">{formatDate(firearm.dispositionDate)}</Fact>
                  <Fact label="Price received">{formatDollars(firearm.dispositionPrice)}</Fact>
                </>
              )}
            </dl>
            <DispositionHistoryList entries={firearm.dispositionHistory} />
            <p className="hd-record__stamp">
              Record added {formatDate(firearm.createdAt.slice(0, 10))}
              {firearm.updatedAt !== firearm.createdAt &&
                ` · last changed ${formatDate(firearm.updatedAt.slice(0, 10))}`}
            </p>
          </section>
        </div>

        <aside className="hd-record__side">
          <section className="hd-panel" aria-labelledby="coverage-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="coverage-title">
                Insurance
              </h2>
              {!disposed && (
                <Button size="sm" icon="shield" onClick={() => setDialog("coverage")}>
                  {policy ? "Change" : "Assign"}
                </Button>
              )}
            </header>
            <div className={`hd-coverage hd-coverage--${coverage.tone}`}>
              <p className="hd-coverage__label">
                <Icon
                  name={
                    coverage.tone === "warn" ? "alert" : coverage.tone === "ok" ? "check" : "shield"
                  }
                  size={18}
                />
                {coverage.label}
              </p>
              <p className="hd-coverage__detail">{coverage.detail}</p>
            </div>
            {coveringPolicy && (
              <dl className="hd-facts hd-facts--compact">
                <Fact label="Policy">
                  <button
                    type="button"
                    className="hd-link"
                    onClick={() => open({ page: "policy", id: coveringPolicy.id })}
                  >
                    {coveringPolicy.name}
                  </button>
                </Fact>
                <Fact label="Coverage">
                  {policy
                    ? `Scheduled, ${formatDollars(firearm.scheduledCoverageAmount)}`
                    : "Blanket, not scheduled"}
                </Fact>
                <Fact label="Term">{expiryLabel(coveringPolicy)}</Fact>
              </dl>
            )}
          </section>

          <DocumentList firearmId={firearm.id} />
        </aside>
      </div>

      <RunningHead
        anchor={plate}
        headingId="record-name"
        title={name}
        stamp={firearm.serialNumber}
        actions={actions("sm")}
      />

      <Dialog
        open={dialog === "edit"}
        onOpenChange={(open) => !open && setDialog(null)}
        title={`Edit ${name}`}
        size="lg"
        bare
      >
        <FirearmForm
          initialValues={firearm}
          focusField={editFocus}
          onSubmit={handleUpdate}
          onCancel={() => setDialog(null)}
        />
      </Dialog>

      <DisposeDialog
        open={dialog === "dispose"}
        onOpenChange={(open) => !open && setDialog(null)}
        firearm={firearm}
        onDispose={handleDispose}
      />

      <RestoreDialog
        open={dialog === "restore"}
        onOpenChange={(open) => !open && setDialog(null)}
        firearm={firearm}
        onRestore={handleRestore}
      />

      <CoverageDialog
        open={dialog === "coverage"}
        onOpenChange={(open) => !open && setDialog(null)}
        firearm={firearm}
        onSave={handleCoverage}
      />

      <ConfirmDialog
        open={dialog === "delete"}
        onOpenChange={(open) => !open && setDialog(null)}
        title={`Delete ${name}?`}
        description={
          disposed
            ? "This erases the record entirely, including its photos, documents, and disposition history. It can't be undone."
            : "This erases the record entirely, including its photos and documents. It can't be undone. If you sold or transferred it, mark it disposed instead to keep its history."
        }
        confirmLabel="Delete firearm"
        onConfirm={handleDelete}
      />
    </div>
  );
}

/** The plate's picture: the designated photo at full resolution (the
 * small thumbnail shows until the original loads), or the type drawing. */
function PlateFigure({
  thumbnailPhotoId,
  typeKey,
}: {
  thumbnailPhotoId: number | null;
  typeKey: string;
}) {
  const [original, setOriginal] = useState<string | null>(null);

  useEffect(() => {
    setOriginal(null);
    if (thumbnailPhotoId == null) return;
    let url: string | null = null;
    let cancelled = false;
    mediaService
      .getPhotoOriginal(thumbnailPhotoId)
      .then((buffer) => {
        if (cancelled) return;
        url = URL.createObjectURL(new Blob([buffer]));
        setOriginal(url);
      })
      .catch(() => {
        // Keep showing the thumbnail.
      });
    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
    };
  }, [thumbnailPhotoId]);

  return (
    <div className="hd-plate__figure">
      <FirearmThumbnail
        className="hd-plate__thumb"
        thumbnailPhotoId={thumbnailPhotoId}
        genericThumbnailKey={typeKey}
      />
      {original && <img className="hd-plate__photo" src={original} alt="" />}
    </div>
  );
}

/** specs/002-firearm-identification: the country a re-imported firearm
 * displays (and searches) is always "United States", never stored
 * (data-model.md's "Derived display values"). */
function countryOfManufactureDisplay(firearm: Firearm): string | null {
  return firearm.origin === "reimported" ? "United States" : firearm.countryOfManufacture;
}

/** specs/002-firearm-identification US2-2: the "Original maker's marks"
 * block renders only when at least one of the three values is recorded. */
function hasOriginalMarks(firearm: Firearm): boolean {
  return Boolean(firearm.originalMake || firearm.originalModel || firearm.originalSerialNumber);
}

/** Whether any of FR-039's six optional details is recorded, so a record
 * with none shows no empty panel. */
function hasPhysicalDetails(firearm: Firearm): boolean {
  return (
    firearm.barrelLengthHundredths != null ||
    firearm.overallLengthHundredths != null ||
    firearm.weightTenthsOz != null ||
    firearm.capacity != null ||
    Boolean(firearm.finish) ||
    firearm.condition != null
  );
}

function TitleCell({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="hd-titleblock__cell">
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  const empty = children == null || children === false || children === "" || children === "—";
  return (
    <div className="hd-facts__row">
      <dt>{label}</dt>
      <dd className={empty ? "hd-muted" : undefined}>{empty ? "Not recorded" : children}</dd>
    </div>
  );
}

function TextBlock({
  text,
  empty,
  onAdd,
}: {
  text: string | null;
  empty: string;
  onAdd: () => void;
}) {
  if (!text) {
    return (
      <p className="hd-panel__empty">
        {empty}{" "}
        <button type="button" className="hd-link" onClick={onAdd}>
          Add
        </button>
      </p>
    );
  }
  return <p className="hd-textblock">{text}</p>;
}
