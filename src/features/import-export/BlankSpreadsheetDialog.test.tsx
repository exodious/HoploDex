import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { render, TEST_SESSION_ID } from "../../test/sessionScope";
import { CommandFailure } from "../../services/tauriClient";
import { BlankSpreadsheetDialog } from "./BlankSpreadsheetDialog";
import * as importExportService from "./importExportService";

// 015 US1, contracts/ui.md §2: the blank import spreadsheet's dialog.

const SCOPE = expect.objectContaining({ id: TEST_SESSION_ID });
vi.mock("./importExportService");

async function chooseFolder(user: ReturnType<typeof userEvent.setup>, path: string) {
  vi.mocked(importExportService.chooseExportFolder).mockResolvedValueOnce({
    token: `token:${path}`,
    path,
  });
  await user.click(screen.getByRole("button", { name: "Choose a folder…" }));
}

function renderDialog() {
  const onOpenChange = vi.fn();
  render(<BlankSpreadsheetDialog open onOpenChange={onOpenChange} />);
  return { dialog: screen.getByRole("dialog", { name: "Get a blank spreadsheet" }), onOpenChange };
}

beforeEach(() => {
  vi.mocked(importExportService.saveImportTemplate).mockReset();
  vi.mocked(importExportService.chooseExportFolder).mockReset();
});

describe("015 US1 - Get a blank spreadsheet to fill in", () => {
  it("offers both formats with Excel selected, and says nothing of the collection leaves (015 US1-1, FR-002, FR-008)", () => {
    const { dialog } = renderDialog();

    expect(within(dialog).getByRole("radio", { name: /Excel \(\.xlsx\)/ })).toBeChecked();
    expect(within(dialog).getByRole("radio", { name: /CSV \(\.csv\)/ })).not.toBeChecked();
    expect(dialog).toHaveTextContent("It holds nothing from your collection.");
    expect(within(dialog).queryByRole("note")).not.toBeInTheDocument();
    expect(dialog).not.toHaveTextContent("not encrypted");
  });

  it("asks for a folder before saving, and saves nothing without one (015 US1-6)", async () => {
    const user = userEvent.setup();
    renderDialog();

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(
      await screen.findByText("Choose the folder to save the spreadsheet in."),
    ).toBeInTheDocument();
    expect(importExportService.saveImportTemplate).not.toHaveBeenCalled();
  });

  it("sends the format and the folder's token, and names the workbook it wrote (015 US1-2)", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.saveImportTemplate).mockResolvedValue({
      spreadsheetPath: "/out/hoplodex-import-template.xlsx",
      accessorySpreadsheetPath: null,
    });
    const { onOpenChange } = renderDialog();

    await chooseFolder(user, "/out");
    expect(importExportService.chooseExportFolder).toHaveBeenCalledWith(SCOPE);
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(importExportService.saveImportTemplate).toHaveBeenCalledWith(SCOPE, {
      format: "xlsx",
      folderToken: "token:/out",
    });
    expect(await screen.findByText("Saved a blank spreadsheet.")).toBeInTheDocument();
    expect(screen.getByText("Spreadsheet")).toBeInTheDocument();
    expect(screen.getByText("/out/hoplodex-import-template.xlsx")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("names both CSV files (015 US1-2)", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.saveImportTemplate).mockResolvedValue({
      spreadsheetPath: "/out/hoplodex-import-template.csv",
      accessorySpreadsheetPath: "/out/hoplodex-import-template-accessories.csv",
    });
    renderDialog();

    await user.click(screen.getByRole("radio", { name: /CSV \(\.csv\)/ }));
    await chooseFolder(user, "/out");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(importExportService.saveImportTemplate).toHaveBeenCalledWith(SCOPE, {
      format: "csv",
      folderToken: "token:/out",
    });
    expect(await screen.findByText("Firearms")).toBeInTheDocument();
    expect(screen.getByText("Accessories")).toBeInTheDocument();
    expect(screen.getByText("/out/hoplodex-import-template-accessories.csv")).toBeInTheDocument();
  });

  it("shows a failure and stays open with the folder kept, for another try (015 FR-009)", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.saveImportTemplate).mockRejectedValue(
      new CommandFailure({ code: "DISK_FULL", message: "The disk is full." }),
    );
    renderDialog();
    await chooseFolder(user, "/out");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("The disk is full.");
    expect(screen.getByText("/out")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  });

  it("is a form whose submission saves, so Enter in it saves (015 FR-011, the One Enter rule)", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.saveImportTemplate).mockResolvedValue({
      spreadsheetPath: "/out/hoplodex-import-template.xlsx",
      accessorySpreadsheetPath: null,
    });
    renderDialog();
    await chooseFolder(user, "/out");

    screen.getByRole("button", { name: "Save" }).closest("form")!.requestSubmit();

    expect(await screen.findByText("Saved a blank spreadsheet.")).toBeInTheDocument();
  });
});
