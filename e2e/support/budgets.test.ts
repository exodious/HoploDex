// @vitest-environment node
// The CI timing skip (#25): budgets are checked unless CI is "true".
import { describe, expect, it } from "vitest";
import { budgetsChecked } from "./budgets";

describe("budgetsChecked", () => {
  it("checks budgets outside CI, and skips them only when CI is true", () => {
    expect(budgetsChecked({})).toBe(true);
    expect(budgetsChecked({ CI: "false" })).toBe(true);
    expect(budgetsChecked({ CI: "" })).toBe(true);
    expect(budgetsChecked({ CI: "true" })).toBe(false);
  });
});
