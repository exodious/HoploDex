import { Dialog, useToast } from "../../components";
import { AccessoryForm } from "../accessories/AccessoryForm";
import * as accessoriesService from "../accessories/accessoriesService";
import type { AccessoryInput } from "../accessories/types";
import { useAccessoryKinds, useCollection } from "../app/collectionStore";
import { accessoryNameText } from "./recordNames";
import type { RecordLabel } from "./types";

// specs/006-accessory-links contracts/ui-accessories.md §3, §5: "Mount → New
// accessory…" on a record's page. The title is still "Add accessory"; Mounted
// on starts as the record whose page this is, and can be changed or cleared.
// The user stays on that page, where the new accessory appears in its Mounted
// section.

export interface NewAccessoryDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The record whose page this is. */
  record: RecordLabel;
}

export function NewAccessoryDialog({ open, onOpenChange, record }: NewAccessoryDialogProps) {
  const { refresh } = useCollection();
  const { kinds } = useAccessoryKinds();
  const notify = useToast();

  async function handleCreate(input: AccessoryInput) {
    const created = await accessoriesService.createAccessory(input);
    onOpenChange(false);
    await refresh();
    const kindName = kinds.find((kind) => kind.id === created.accessoryKindId)?.name;
    notify(
      `Added ${accessoryNameText(created.make, created.model, kindName ?? "accessory")} to the accessories.`,
    );
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange} title="Add accessory" size="lg" bare>
      <AccessoryForm
        presetMountedOn={record}
        onSubmit={handleCreate}
        onCancel={() => onOpenChange(false)}
      />
    </Dialog>
  );
}
