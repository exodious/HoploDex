import { describe, expect, it } from "vitest";
import { caliberHint, caliberReducer, initialCaliberState } from "./caliberDerivation";
import type { CaliberAction, CaliberState } from "./caliberDerivation";

// specs/004-cartridges-action-types research.md §8 and contracts/ui-entry.md
// §3's table: every transition, from both starting modes.

const catalog9mm = { caliber: "9mm", source: "catalog" } as const;
const guess30 = { caliber: ".30", source: "guess" } as const;
const catalog45 = { caliber: ".45", source: "catalog" } as const;
const catalog30 = { caliber: ".30", source: "catalog" } as const;

/** Runs the actions for a type that derives its caliber (004's every type). */
function run(state: CaliberState, ...actions: CaliberAction[]): CaliberState {
  return actions.reduce((prev, action) => caliberReducer(prev, action, true), state);
}

/** Runs the actions for a Suppressor, whose caliber is never derived. */
function runBore(state: CaliberState, ...actions: CaliberAction[]): CaliberState {
  return actions.reduce((prev, action) => caliberReducer(prev, action, false), state);
}

const typeChanged: CaliberAction = { type: "typeChanged" };

const settled = (
  cartridge: string,
  derived: { caliber: string; source: "catalog" | "guess" } | null,
): CaliberAction => ({ type: "cartridgeSettled", cartridge, derived });

describe("initialCaliberState", () => {
  it("starts a new firearm derived and empty", () => {
    expect(initialCaliberState()).toEqual({
      caliber: "",
      mode: "derived",
      source: null,
      suggestion: null,
      prompt: null,
      derivedFrom: null,
    });
  });

  it("starts a saved firearm edited, keeping its caliber (FR-006)", () => {
    const state = initialCaliberState("9mm");
    expect(state.caliber).toBe("9mm");
    expect(state.mode).toBe("edited");
    expect(state.source).toBeNull();
  });
});

describe("caliberReducer, derived mode", () => {
  const fresh = initialCaliberState();

  it("takes a catalog cartridge's caliber (US1-1)", () => {
    const state = run(fresh, settled("9x19mm Parabellum", catalog9mm));
    expect(state).toMatchObject({ caliber: "9mm", mode: "derived", source: "catalog" });
    expect(state.suggestion).toBeNull();
    expect(caliberHint(state)).toBe("From the cartridge.");
  });

  it("takes a guess, marked as one (US1-2)", () => {
    const state = run(fresh, settled(".30 Custom Improved", guess30));
    expect(state).toMatchObject({ caliber: ".30", mode: "derived", source: "guess" });
    expect(caliberHint(state)).toBe("Guessed from the cartridge. Check it before saving.");
  });

  it("empties the caliber and asks for it when nothing can be read (US1-4)", () => {
    const state = run(
      fresh,
      settled("9x19mm Parabellum", catalog9mm),
      settled("Wildcat Special", null),
    );
    expect(state).toMatchObject({
      caliber: "",
      mode: "derived",
      source: null,
      prompt: "Wildcat Special",
    });
    expect(caliberHint(state)).toBe(
      "We couldn't work out a caliber from “Wildcat Special”. Enter it.",
    );
  });

  it("empties the caliber with no hint when the cartridge is cleared", () => {
    const state = run(fresh, settled("Wildcat Special", null), { type: "cartridgeCleared" });
    expect(state).toMatchObject({ caliber: "", mode: "derived", source: null, prompt: null });
    expect(caliberHint(state)).toBeUndefined();

    const fromCatalog = run(fresh, settled(".45 ACP", catalog45), { type: "cartridgeCleared" });
    expect(fromCatalog.caliber).toBe("");
    expect(caliberHint(fromCatalog)).toBeUndefined();
  });

  it("re-derives on every cartridge change until the caliber is edited", () => {
    const state = run(
      fresh,
      settled("9x19mm Parabellum", catalog9mm),
      settled(".45 ACP", catalog45),
    );
    expect(state.caliber).toBe(".45");
  });

  it("becomes edited when the caliber is typed in, dropping the tag and hint (US1-3)", () => {
    const state = run(fresh, settled(".30 Custom Improved", guess30), {
      type: "caliberTyped",
      caliber: ".308",
    });
    expect(state).toMatchObject({ caliber: ".308", mode: "edited", source: null, prompt: null });
    expect(caliberHint(state)).toBeUndefined();

    // US1-3: a later cartridge change leaves the typed caliber alone.
    const later = run(state, settled("9x19mm Parabellum", catalog9mm));
    expect(later.caliber).toBe(".308");
  });

  it("typing in the caliber removes the couldn't-work-it-out prompt", () => {
    const state = run(fresh, settled("Wildcat Special", null), {
      type: "caliberTyped",
      caliber: ".3",
    });
    expect(state.prompt).toBeNull();
  });

  it("leaving the caliber empty stays derived and re-derives at once", () => {
    const state = run(
      fresh,
      settled("9x19mm Parabellum", catalog9mm),
      { type: "caliberTyped", caliber: "" },
      { type: "caliberLeft" },
    );
    expect(state).toMatchObject({ caliber: "9mm", mode: "derived", source: "catalog" });
  });

  it("leaving an untouched empty caliber with no cartridge changes nothing", () => {
    expect(run(fresh, { type: "caliberLeft" })).toEqual(fresh);
  });
});

describe("caliberReducer, edited mode (a saved firearm)", () => {
  const saved = initialCaliberState("9mm");

  it("keeps the caliber and offers the derived one when it differs (US1-7)", () => {
    const state = run(saved, settled(".45 ACP", catalog45));
    expect(state).toMatchObject({ caliber: "9mm", mode: "edited", suggestion: ".45" });
    expect(caliberHint(state)).toBeUndefined();
  });

  it("offers a guess the same way", () => {
    expect(run(saved, settled(".30 Custom Improved", guess30)).suggestion).toBe(".30");
  });

  it("offers nothing when the derived caliber is the one already there", () => {
    expect(run(saved, settled("9x19mm Parabellum", catalog9mm)).suggestion).toBeNull();
  });

  it("is unchanged, with no suggestion, for a name with no readable bore", () => {
    const state = run(saved, settled(".45 ACP", catalog45), settled("Wildcat Special", null));
    expect(state).toMatchObject({ caliber: "9mm", suggestion: null, prompt: null });
  });

  it("is unchanged, with no suggestion, when the cartridge is cleared", () => {
    const state = run(saved, settled(".45 ACP", catalog45), { type: "cartridgeCleared" });
    expect(state).toMatchObject({ caliber: "9mm", mode: "edited", suggestion: null });
  });

  it("using the suggestion sets the caliber and stays edited", () => {
    const state = run(saved, settled(".45 ACP", catalog45), { type: "suggestionUsed" });
    expect(state).toMatchObject({ caliber: ".45", mode: "edited", suggestion: null, source: null });
  });

  it("drops the suggestion once the caliber is typed to the suggested value", () => {
    const offered = run(saved, settled(".45 ACP", catalog45));
    expect(run(offered, { type: "caliberTyped", caliber: ".4" }).suggestion).toBe(".45");
    expect(run(offered, { type: "caliberTyped", caliber: ".45" }).suggestion).toBeNull();
  });

  it("returns to derived when the caliber is left empty, then derives from the cartridge", () => {
    const state = run(
      saved,
      settled(".45 ACP", catalog45),
      { type: "caliberTyped", caliber: "" },
      { type: "caliberLeft" },
    );
    expect(state).toMatchObject({
      caliber: ".45",
      mode: "derived",
      source: "catalog",
      suggestion: null,
    });

    const unreadable = run(
      saved,
      settled("Wildcat Special", null),
      { type: "caliberTyped", caliber: "" },
      { type: "caliberLeft" },
    );
    expect(unreadable).toMatchObject({ caliber: "", mode: "derived", prompt: "Wildcat Special" });
  });

  it("a cartridge settled after returning to derived fills the caliber again", () => {
    const state = run(
      saved,
      { type: "caliberTyped", caliber: "" },
      { type: "caliberLeft" },
      settled(".30 Custom Improved", guess30),
    );
    expect(state).toMatchObject({ caliber: ".30", mode: "derived", source: "guess" });
  });
});

// specs/005-regulated-item-types FR-002, US1-7, research.md §15: a type
// whose `caliberFromCartridge` is false (Suppressor) never derives its
// caliber; changing type keeps the caliber's text either way.
describe("caliberReducer, a type that doesn't derive (Suppressor)", () => {
  const fresh = initialCaliberState();
  const saved = initialCaliberState(".30");

  it("remembers a settled cartridge but fills nothing on a new firearm", () => {
    const state = runBore(fresh, settled(".300 Winchester Magnum", catalog30));
    expect(state).toMatchObject({ caliber: "", mode: "derived", source: null, prompt: null });
    expect(state.suggestion).toBeNull();
    expect(state.derivedFrom).toEqual({
      cartridge: ".300 Winchester Magnum",
      derived: catalog30,
    });
    expect(caliberHint(state)).toBeUndefined();
  });

  it("neither guesses nor prompts for a cartridge it can't read", () => {
    expect(runBore(fresh, settled(".30 Custom Improved", guess30))).toMatchObject({
      caliber: "",
      source: null,
    });
    const unreadable = runBore(fresh, settled("Wildcat Special", null));
    expect(unreadable.prompt).toBeNull();
    expect(caliberHint(unreadable)).toBeUndefined();
  });

  it("suggests nothing for a saved suppressor's new rated cartridge", () => {
    const state = runBore(saved, settled(".300 Winchester Magnum", catalog30));
    expect(state).toMatchObject({ caliber: ".30", mode: "edited", suggestion: null });
  });

  it("derives nothing when an empty caliber is left", () => {
    const state = runBore(
      saved,
      settled(".300 Winchester Magnum", catalog30),
      { type: "caliberTyped", caliber: "" },
      { type: "caliberLeft" },
    );
    expect(state).toMatchObject({ caliber: "", mode: "derived", source: null, prompt: null });
  });

  it("keeps a typed bore when the rated cartridge is cleared", () => {
    const state = runBore(
      fresh,
      settled(".300 Winchester Magnum", catalog30),
      { type: "caliberTyped", caliber: ".46" },
      { type: "cartridgeCleared" },
    );
    expect(state).toMatchObject({ caliber: ".46", mode: "edited", derivedFrom: null });
  });
});

describe("caliberReducer, changing type", () => {
  const fresh = initialCaliberState();

  it("to a Suppressor keeps a derived caliber as the owner's, without its hint", () => {
    const derived = run(fresh, settled("9x19mm Parabellum", catalog9mm));
    const state = runBore(derived, typeChanged);
    expect(state).toMatchObject({ caliber: "9mm", mode: "edited", source: null });
    expect(caliberHint(state)).toBeUndefined();
  });

  it("to a Suppressor drops a guess's tag and a pending suggestion", () => {
    const guessed = run(fresh, settled(".30 Custom Improved", guess30));
    expect(runBore(guessed, typeChanged)).toMatchObject({
      caliber: ".30",
      mode: "edited",
      source: null,
    });
    const offered = run(initialCaliberState("9mm"), settled(".45 ACP", catalog45));
    expect(offered.suggestion).toBe(".45");
    expect(runBore(offered, typeChanged).suggestion).toBeNull();
  });

  it("to a Suppressor drops the prompt and leaves an empty caliber empty", () => {
    const prompted = run(fresh, settled("Wildcat Special", null));
    const state = runBore(prompted, typeChanged);
    expect(state).toMatchObject({ caliber: "", prompt: null });
    expect(caliberHint(state)).toBeUndefined();
  });

  it("away from a Suppressor derives an empty caliber from the last cartridge at once", () => {
    const bore = runBore(fresh, settled(".300 Winchester Magnum", catalog30));
    const state = run(bore, typeChanged);
    expect(state).toMatchObject({ caliber: ".30", mode: "derived", source: "catalog" });
    expect(caliberHint(state)).toBe("From the cartridge.");

    const unreadable = run(runBore(fresh, settled("Wildcat Special", null)), typeChanged);
    expect(unreadable).toMatchObject({ caliber: "", mode: "derived", prompt: "Wildcat Special" });
  });

  it("away from a Suppressor leaves a non-empty caliber edited", () => {
    const bore = runBore(fresh, settled(".300 Winchester Magnum", catalog30), {
      type: "caliberTyped",
      caliber: ".46",
    });
    const state = run(bore, typeChanged);
    expect(state).toMatchObject({ caliber: ".46", mode: "edited", suggestion: null });
    // A later cartridge change only suggests (004 FR-006).
    expect(run(state, settled(".45 ACP", catalog45))).toMatchObject({
      caliber: ".46",
      suggestion: ".45",
    });
  });

  it("away from a Suppressor with no cartridge leaves an empty caliber empty", () => {
    expect(run(fresh, typeChanged)).toMatchObject({ caliber: "", mode: "derived", prompt: null });
  });
});
