import { useEffect, useState } from "react";
import { Badge, Button, ConfirmDialog, Dialog, Icon, useToast } from "../../components";
import { formatDate } from "../../lib/dates";
import { formatDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { BackLink } from "../app/BackLink";
import { useAccessoryKinds, useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import { RunningHead } from "../app/RunningHead";
import { DispositionHistoryList } from "../firearms/DispositionHistoryList";
import { DisposeDialog } from "../firearms/DisposeDialog";
import { Fact, PlateFigure, TextBlock, TitleCell } from "../firearms/FirearmRecordPage";
import { RestoreDialog } from "../firearms/RestoreDialog";
import { dispositionLabel } from "../firearms/types";
import type { DisposeFirearmInput, ReverseDispositionInput } from "../firearms/types";
import { CoverageDialog } from "../insurance/CoverageDialog";
import { coverageStatus, expiryLabel } from "../insurance/coverage";
import type { AssignCoverageInput } from "../insurance/types";
import { DocumentList } from "../media/DocumentList";
import { PhotoGallery } from "../media/PhotoGallery";
import { MountedOnChain } from "../mounts/MountedOnChain";
import { MountedSection } from "../mounts/MountedSection";
import { NewAccessoryDialog } from "../mounts/NewAccessoryDialog";
import { accessoryNameText } from "../mounts/recordNames";
import type { RecordLabel } from "../mounts/types";
import { peekResumedDraft } from "../session/usePendingDraft";
import { AccessoryForm } from "./AccessoryForm";
import * as accessoriesService from "./accessoriesService";
import type { Accessory, AccessoryDetail, AccessoryInput } from "./types";
import "../firearms/record.css";
import "./accessories.css";

type RecordDialog = "edit" | "dispose" | "restore" | "delete" | "coverage" | "mountNew";

export interface AccessoryRecordPageProps {
  id: number;
}

function failureMessage(e: unknown, fallback: string): string {
  return e instanceof CommandFailure ? e.message : fallback;
}

/** One accessory's full record (specs/006-accessory-links US1, FR-007a,
 * FR-013; contracts/ui-accessories.md §12): the firearm record page's
 * layout, with photos, documents, value, notes, disposition history and
 * coverage, and (User Story 2) its "Mounted on" chain and Mounted section. */
export function AccessoryRecordPage({ id }: AccessoryRecordPageProps) {
  const {
    accessoriesById,
    policiesById,
    summary: valueSummary,
    revision,
    refresh,
  } = useCollection();
  const { kinds } = useAccessoryKinds();
  const { open, back } = useNavigation();
  const notify = useToast();
  const [accessory, setAccessory] = useState<AccessoryDetail | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [dialog, setDialog] = useState<RecordDialog | null>(null);
  // The plate, which the pinned strip waits to scroll away.
  const [plate, setPlate] = useState<HTMLElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    accessoriesService
      .getAccessory(id)
      .then((loaded) => !cancelled && setAccessory(loaded))
      .catch(
        (e) => !cancelled && setLoadError(failureMessage(e, "This accessory couldn't be loaded.")),
      );
    return () => {
      cancelled = true;
    };
  }, [id, revision]);

  // Pending changes the user resumed for this accessory reopen their form
  // (FR-039), which takes them as its unsaved input.
  const loadedId = accessory?.id;
  useEffect(() => {
    const resumed = peekResumedDraft();
    if (loadedId === undefined || resumed?.kind !== "accessory" || resumed.targetId !== loadedId)
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
  if (!accessory) {
    return <div className="hd-record">{back && <BackLink target={back} />}</div>;
  }

  const kind = kinds.find((k) => k.id === accessory.accessoryKindId);
  const kindName = kind?.name ?? "Accessory";
  const name = accessoryNameText(accessory.make, accessory.model, kindName);
  const disposed = accessory.status === "disposed";
  // How mounts name this accessory.
  const label: RecordLabel = {
    record: { kind: "accessory", id: accessory.id },
    make: accessory.make,
    model: accessory.model,
    nickname: null,
    typeName: kindName,
    serialNumber: accessory.serialNumber,
    status: accessory.status,
  };
  const policy =
    accessory.insurancePolicyId != null ? policiesById.get(accessory.insurancePolicyId) : undefined;
  const blanket = valueSummary?.blanket ?? null;
  // An unscheduled accessory is covered by the blanket policy in force.
  const blanketPolicy =
    !policy && blanket && !disposed ? policiesById.get(blanket.policyId) : undefined;
  const coveringPolicy = policy ?? blanketPolicy;
  const coverage = disposed
    ? {
        tone: "neutral" as const,
        label: "Not tracked",
        detail: "Disposed accessories are left out of totals and coverage checks.",
      }
    : coverageStatus(
        accessory,
        accessoriesById.get(accessory.id)?.insuranceWarning ?? "none",
        policy,
        blanket,
      );

  async function afterChange(updated: Accessory, message: string) {
    // The refresh below reloads the retained history; keep what's shown
    // until it arrives.
    setAccessory((current) => ({
      ...(current as AccessoryDetail),
      ...updated,
    }));
    setDialog(null);
    await refresh();
    notify(message);
  }

  async function handleUpdate(input: AccessoryInput) {
    const updated = await accessoriesService.updateAccessory(id, input);
    await afterChange(updated, `Saved changes to ${name}.`);
  }

  async function handleDispose(input: DisposeFirearmInput) {
    const updated = await accessoriesService.disposeAccessory(id, input);
    await afterChange(
      updated,
      `Marked ${name} as ${dispositionLabel(updated.dispositionType).toLowerCase()}.`,
    );
  }

  async function handleRestore(input: ReverseDispositionInput) {
    const restored = await accessoriesService.reverseAccessoryDisposition(id, {
      history: input.history,
    });
    await afterChange(restored, `Restored ${name} to the collection.`);
  }

  async function handleCoverage(input: AssignCoverageInput) {
    const updated = await accessoriesService.assignAccessoryCoverage(
      id,
      input.policyId,
      input.scheduledCoverageAmount ?? null,
    );
    await afterChange(updated, `Updated coverage for ${name}.`);
  }

  async function handleDelete() {
    try {
      await accessoriesService.deleteAccessory(id, true);
      await refresh();
      notify(`Deleted ${name}.`);
      back?.go();
    } catch (e) {
      notify(failureMessage(e, `${name} couldn't be deleted.`), "error");
    }
  }

  async function handleMediaChanged() {
    setAccessory(await accessoriesService.getAccessory(id));
    await refresh();
  }

  function actions(size: "md" | "sm") {
    return (
      <>
        <Button size={size} icon="pencil" onClick={() => setDialog("edit")}>
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
          <PlateFigure
            thumbnailPhotoId={accessory.thumbnailPhotoId}
            typeKey={kind?.genericThumbnailKey ?? "accessory"}
          />
          <header className="hd-plate__heading">
            <p className="hd-eyebrow hd-plate__type">
              {kindName}
              {disposed && (
                <Badge tone="neutral" icon="archive">
                  {dispositionLabel(accessory.dispositionType)}
                </Badge>
              )}
            </p>
            <h1 className="hd-plate__name" id="record-name" tabIndex={-1}>
              {name}
            </h1>
            <p className="hd-plate__stamp">
              <span className="hd-plate__stamp-label">Serial no.</span>
              {accessory.serialNumber ? (
                <span className="hd-serial hd-plate__serial">{accessory.serialNumber}</span>
              ) : (
                <span className="hd-plate__no-serial">None recorded</span>
              )}
            </p>
          </header>
        </div>

        <dl className="hd-titleblock">
          <TitleCell label="Cartridge">{accessory.cartridge ?? "—"}</TitleCell>
          <TitleCell label="Caliber">{accessory.caliber ?? "—"}</TitleCell>
          <TitleCell label="Status">
            {disposed
              ? `${dispositionLabel(accessory.dispositionType)} ${formatDate(accessory.dispositionDate)}`
              : "Active"}
          </TitleCell>
          <TitleCell label="Replacement value">
            <span className="hd-num">{formatDollars(accessory.estimatedValue)}</span>
          </TitleCell>
          <TitleCell label="Acquired">{formatDate(accessory.acquisitionDate)}</TitleCell>
          <TitleCell label="Coverage">
            <Badge tone={coverage.tone}>{coverage.label}</Badge>
          </TitleCell>
        </dl>
      </article>

      <div className="hd-record__grid">
        <div className="hd-record__main">
          <PhotoGallery
            owner={{ kind: "accessory", id: accessory.id }}
            thumbnailPhotoId={accessory.thumbnailPhotoId}
            onChanged={handleMediaChanged}
          />

          <section className="hd-panel" aria-labelledby="details-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="details-title">
                Details
              </h2>
            </header>
            <dl className="hd-facts">
              <Fact label="Kind">{kindName}</Fact>
              <Fact label="Make">{accessory.make}</Fact>
              <Fact label="Model">{accessory.model}</Fact>
              <Fact label="Serial number">{accessory.serialNumber}</Fact>
              <Fact label="Caliber">{accessory.caliber}</Fact>
              <Fact label="Cartridge">{accessory.cartridge}</Fact>
              {accessory.mount.chain.length > 0 && (
                <Fact label="Mounted on">
                  <MountedOnChain chain={accessory.mount.chain} />
                </Fact>
              )}
            </dl>
          </section>

          <section className="hd-panel" aria-labelledby="value-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="value-title">
                Value and acquisition
              </h2>
            </header>
            <dl className="hd-facts">
              <Fact label="Replacement value">
                {accessory.estimatedValue != null && formatDollars(accessory.estimatedValue)}
              </Fact>
              <Fact label="Acquired from">{accessory.acquisitionSource}</Fact>
              <Fact label="Date acquired">
                {accessory.acquisitionDate && formatDate(accessory.acquisitionDate)}
              </Fact>
              <Fact label="Price paid">
                {accessory.acquisitionPrice != null && formatDollars(accessory.acquisitionPrice)}
              </Fact>
            </dl>
          </section>

          <section className="hd-panel" aria-labelledby="notes-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="notes-title">
                Notes
              </h2>
            </header>
            <TextBlock
              text={accessory.notes}
              empty="No notes recorded."
              onAdd={() => setDialog("edit")}
            />
          </section>

          {/* specs/006-accessory-links FR-012: after the notes, before the documents. */}
          {!disposed && (
            <MountedSection
              record={label}
              mounted={accessory.mount.mounted}
              onNewAccessory={() => setDialog("mountNew")}
            />
          )}

          <DocumentList owner={{ kind: "accessory", id: accessory.id }} />

          <section className="hd-panel" aria-labelledby="history-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="history-title">
                Disposition history
              </h2>
            </header>
            {disposed ? (
              <dl className="hd-facts">
                <Fact label="Disposition">{dispositionLabel(accessory.dispositionType)}</Fact>
                <Fact label="Transferred to">{accessory.dispositionRecipient}</Fact>
                <Fact label="Date">{formatDate(accessory.dispositionDate)}</Fact>
                <Fact label="Price received">{formatDollars(accessory.dispositionPrice)}</Fact>
              </dl>
            ) : (
              accessory.dispositionHistory.length === 0 && (
                <p className="hd-panel__empty">Never disposed of.</p>
              )
            )}
            <DispositionHistoryList entries={accessory.dispositionHistory} />
            <p className="hd-record__stamp">
              Record added {formatDate(accessory.createdAt.slice(0, 10))}
              {accessory.updatedAt !== accessory.createdAt &&
                ` · last changed ${formatDate(accessory.updatedAt.slice(0, 10))}`}
            </p>
          </section>
        </div>

        <aside className="hd-record__side">
          <section className="hd-panel" aria-labelledby="coverage-title">
            <header className="hd-panel__head">
              <h2 className="hd-panel__title" id="coverage-title">
                Coverage
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
                    ? `Scheduled, ${formatDollars(accessory.scheduledCoverageAmount)}`
                    : "Blanket, not scheduled"}
                </Fact>
                <Fact label="Term">{expiryLabel(coveringPolicy)}</Fact>
              </dl>
            )}
          </section>
        </aside>
      </div>

      <RunningHead
        anchor={plate}
        headingId="record-name"
        title={name}
        stamp={accessory.serialNumber}
        actions={actions("sm")}
      />

      <Dialog
        open={dialog === "edit"}
        onOpenChange={(isOpen) => !isOpen && setDialog(null)}
        title={`Edit ${name}`}
        size="lg"
        bare
      >
        <AccessoryForm
          initialValues={accessory}
          onSubmit={handleUpdate}
          onCancel={() => setDialog(null)}
        />
      </Dialog>

      <DisposeDialog
        open={dialog === "dispose"}
        onOpenChange={(isOpen) => !isOpen && setDialog(null)}
        accessory={accessory}
        onDispose={handleDispose}
      />

      <RestoreDialog
        open={dialog === "restore"}
        onOpenChange={(isOpen) => !isOpen && setDialog(null)}
        accessory={accessory}
        onRestore={handleRestore}
      />

      <CoverageDialog
        open={dialog === "coverage"}
        onOpenChange={(isOpen) => !isOpen && setDialog(null)}
        accessory={accessory}
        onSave={handleCoverage}
      />

      <NewAccessoryDialog
        open={dialog === "mountNew"}
        onOpenChange={(isOpen) => !isOpen && setDialog(null)}
        record={label}
      />

      <ConfirmDialog
        open={dialog === "delete"}
        onOpenChange={(isOpen) => !isOpen && setDialog(null)}
        title={`Delete ${name}?`}
        description={
          disposed
            ? "This erases the record entirely, including its photos, documents, and disposition history. It can't be undone."
            : "This erases the record entirely, including its photos and documents. It can't be undone. If you sold or transferred it, mark it disposed instead to keep its history."
        }
        confirmLabel="Delete accessory"
        onConfirm={handleDelete}
      />
    </div>
  );
}
