import { describe, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";
import { createSessionScope } from "../session/sessionScope";
import { render, SessionScopeWrapper } from "../../test/sessionScope";
import { CollectionProvider } from "./CollectionProvider";

const scoped = vi.hoisted(() => ({ calls: [] as Array<[string, unknown]> }));

function record(name: string, result: unknown) {
  return (scope: unknown) => {
    scoped.calls.push([name, scope]);
    return Promise.resolve(result);
  };
}

vi.mock("../browse/browseService", () => ({
  listFirearms: (scope: unknown) => record("listFirearms", { groups: [] })(scope),
}));
vi.mock("../accessories/accessoriesService", () => ({
  listAccessories: record("listAccessories", { groups: [] }),
  listAccessoryKinds: record("listAccessoryKinds", { kinds: [] }),
}));
vi.mock("../firearms/firearmsService", () => ({
  listActionTypes: record("listActionTypes", { actions: [], allowedByFirearmType: {} }),
  listFirearmTypes: record("listFirearmTypes", { types: [] }),
  listRegistrationClasses: record("listRegistrationClasses", { classes: [] }),
}));
vi.mock("../insurance/insuranceService", () => ({
  getValueSummary: record("getValueSummary", null),
  listInsurancePolicies: record("listInsurancePolicies", []),
}));
vi.mock("../media/documentOpening", () => ({ loadDocumentOpening: () => Promise.resolve() }));

describe("CollectionProvider", () => {
  it("loads everything through the scope of the open it renders in", async () => {
    scoped.calls.length = 0;
    const scope = createSessionScope(7);
    render(
      <SessionScopeWrapper scope={scope}>
        <CollectionProvider>{null}</CollectionProvider>
      </SessionScopeWrapper>,
    );

    await waitFor(() => expect(scoped.calls).toHaveLength(8));
    for (const [, used] of scoped.calls) expect(used).toBe(scope);
  });
});
