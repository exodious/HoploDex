import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { Badge, Button, ConfirmDialog, Dialog, Icon, useToast } from "../../components";
import { formatDate } from "../../lib/dates";
import { formatCents } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { BackLink } from "../app/BackLink";
import { firearmName, useCollection } from "../app/collectionStore";
import { FirearmName } from "../app/FirearmName";
import { useNavigation } from "../app/navigation";
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
import { RestoreDialog } from "./RestoreDialog";
import * as firearmsService from "./firearmsService";
import { notifySaveWarnings } from "./saveWarnings";
import { dispositionLabel, firearmTypeOption } from "./types";
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
  const { firearmsById, policiesById, revision, refresh } = useCollection();
  const { open, back } = useNavigation();
  const notify = useToast();
  const [firearm, setFirearm] = useState<FirearmDetail | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [dialog, setDialog] = useState<RecordDialog | null>(null);

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

  // Escape returns to the list, unless it's closing a dialog or popover.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (
        document.querySelector(
          '[role="dialog"], [role="alertdialog"], [data-radix-popper-content-wrapper]',
        )
      )
        return;
      back?.go();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [back]);

  if (loadError) {
    return (
      <div className="hd-record">
        {back && <BackLink target={back} escapes />}
        <p className="hd-banner hd-banner--error" role="alert">
          <Icon name="alert" />
          <span className="hd-banner__text">{loadError}</span>
        </p>
      </div>
    );
  }
  if (!firearm) {
    return <div className="hd-record">{back && <BackLink target={back} escapes />}</div>;
  }

  const name = firearmName(firearm);
  const summary = firearmsById.get(firearm.id);
  const policy =
    firearm.insurancePolicyId != null ? policiesById.get(firearm.insurancePolicyId) : undefined;
  const coverage = coverageStatus(firearm, summary?.insuranceWarning ?? "none", policy);
  const type = firearmTypeOption(firearm.firearmTypeId);
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

  async function handleUpdate(input: FirearmInput) {
    const updated = await firearmsService.updateFirearm(id, input);
    await afterChange(updated, `Saved changes to ${firearmName(updated)}.`);
    notifySaveWarnings(notify, updated.warnings);
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
    notifySaveWarnings(notify, restored.warnings);
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

  return (
    <div className="hd-record">
      <div className="hd-record__bar">
        {back && <BackLink target={back} escapes />}
        <div className="hd-record__actions">
          <Button icon="pencil" onClick={() => setDialog("edit")}>
            Edit
          </Button>
          {disposed ? (
            <Button icon="archive" onClick={() => setDialog("restore")}>
              Restore to collection
            </Button>
          ) : (
            <Button icon="archive" onClick={() => setDialog("dispose")}>
              Mark disposed
            </Button>
          )}
          <Button variant="ghost" icon="trash" onClick={() => setDialog("delete")}>
            Delete
          </Button>
        </div>
      </div>

      <article className="hd-plate" aria-labelledby="record-name">
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
            <h1 className="hd-plate__name" id="record-name">
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
          <TitleCell label="Caliber">{firearm.caliber}</TitleCell>
          <TitleCell label="Status">
            {disposed
              ? `${dispositionLabel(firearm.dispositionType)} ${formatDate(firearm.dispositionDate)}`
              : "Active"}
          </TitleCell>
          <TitleCell label="Replacement value">
            <span className="hd-num">{formatCents(firearm.estimatedValue)}</span>
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

          <section className="hd-panel" aria-labelledby="notes-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="notes-title">
                Condition and notes
              </h2>
            </header>
            <TextBlock
              text={firearm.notes}
              empty="No notes recorded."
              onAdd={() => setDialog("edit")}
            />
            <h3 className="hd-subhead">Accessories</h3>
            <TextBlock
              text={firearm.accessories}
              empty="No accessories recorded."
              onAdd={() => setDialog("edit")}
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
                {firearm.acquisitionPrice != null && formatCents(firearm.acquisitionPrice)}
              </Fact>
              {disposed && (
                <>
                  <Fact label="Disposition">{dispositionLabel(firearm.dispositionType)}</Fact>
                  <Fact label="Transferred to">{firearm.dispositionRecipient}</Fact>
                  <Fact label="Date">{formatDate(firearm.dispositionDate)}</Fact>
                  <Fact label="Price received">{formatCents(firearm.dispositionPrice)}</Fact>
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
            {policy && (
              <dl className="hd-facts hd-facts--compact">
                <Fact label="Policy">
                  <button
                    type="button"
                    className="hd-link"
                    onClick={() => open({ page: "policy", id: policy.id })}
                  >
                    {policy.name}
                  </button>
                </Fact>
                <Fact label="Coverage">
                  {firearm.coverageKind === "individually_scheduled"
                    ? `Scheduled, ${formatCents(firearm.scheduledCoverageAmount)}`
                    : "Blanket"}
                </Fact>
                <Fact label="Term">{expiryLabel(policy.effectiveEndDate)}</Fact>
              </dl>
            )}
          </section>

          <DocumentList firearmId={firearm.id} />
        </aside>
      </div>

      <Dialog
        open={dialog === "edit"}
        onOpenChange={(open) => !open && setDialog(null)}
        title={`Edit ${name}`}
        size="lg"
        bare
      >
        <FirearmForm
          initialValues={firearm}
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
