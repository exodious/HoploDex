import { describe, expect, it } from "vitest";
import { firstRefusedField, focusFormField } from "./formFocus";

describe("focusFormField", () => {
  function formWith(html: string): HTMLFormElement {
    const form = document.createElement("form");
    form.innerHTML = html;
    document.body.append(form);
    return form;
  }

  it("focuses the control inside the field's wrapper and says so", () => {
    const form = formWith(
      '<div data-field="a"><input id="a" /></div><div data-field="b"><textarea id="b"></textarea></div>',
    );

    expect(focusFormField(form, "b")).toBe(true);
    expect(document.activeElement?.id).toBe("b");
    form.remove();
  });

  it("takes the input before a button in the same wrapper", () => {
    const form = formWith('<div data-field="d"><input id="in" /><button id="pick"></button></div>');

    focusFormField(form, "d");
    expect(document.activeElement?.id).toBe("in");
    form.remove();
  });

  it("does nothing for a field the form lacks, or no form", () => {
    const form = formWith('<div data-field="a"><input id="a" /></div>');

    expect(focusFormField(form, "missing")).toBe(false);
    expect(focusFormField(null, "a")).toBe(false);
    form.remove();
  });
});

describe("firstRefusedField", () => {
  const order = ["make", "model", "weightPounds"] as const;

  it("returns the first field in the form's order, not the error object's", () => {
    expect(firstRefusedField(order, { weightPounds: "x", model: "y" })).toBe("model");
  });

  it("reads a field under the name the backend gives it", () => {
    expect(
      firstRefusedField(order, { weightTenthsOz: "x" }, { weightPounds: "weightTenthsOz" }),
    ).toBe("weightPounds");
  });

  it("returns nothing when no listed field was refused", () => {
    expect(firstRefusedField(order, { other: "x" })).toBeUndefined();
    expect(firstRefusedField(order, undefined)).toBeUndefined();
  });
});
