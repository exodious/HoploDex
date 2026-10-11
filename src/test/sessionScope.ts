import { render as testingLibraryRender } from "@testing-library/react";
import type { RenderOptions, RenderResult } from "@testing-library/react";
import { createElement } from "react";
import type { ReactElement, ReactNode } from "react";
import type { Draft } from "../features/databases/types";
import { createSessionScope, SessionScopeContext } from "../features/session/sessionScope";
import type { SessionScope } from "../features/session/sessionScope";

/** The session id of {@link DEFAULT_SCOPE}. */
export const TEST_SESSION_ID = 1;

/** The scope `render` below provides when a test passes none. A service mocked
 * with `vi.mock` receives it as its first argument. */
export const DEFAULT_SCOPE: SessionScope = createSessionScope(TEST_SESSION_ID);

/** Wraps children in a live `SessionScope` (specs/008 research.md §3), as
 * `SessionProvider` does for the collection's tree. Pass `scope` to share one
 * across renders or to end it mid-test. */
export function SessionScopeWrapper({
  scope = DEFAULT_SCOPE,
  children,
}: {
  scope?: SessionScope;
  children?: ReactNode;
}): ReactElement {
  return createElement(SessionScopeContext.Provider, { value: scope }, children);
}

/** Testing Library's `render`, inside the default session scope. An explicit
 * `wrapper` option still applies, inside the scope. */
export function render(
  ui: ReactElement,
  options: Omit<RenderOptions, "queries"> = {},
): RenderResult {
  const Inner = options.wrapper;
  return testingLibraryRender(ui, {
    ...options,
    wrapper: ({ children }: { children?: ReactNode }) =>
      createElement(
        SessionScopeWrapper,
        null,
        Inner ? createElement(Inner, null, children) : children,
      ),
  });
}

/** Puts `draft` in `scope` as the pending changes the user chose to resume,
 * for a form to open with (specs/008 research.md §6), or takes it out with
 * `null`. The backend isn't told when the form opens. */
export function setResumedDraft(draft: Draft | null, scope: SessionScope = DEFAULT_SCOPE): void {
  scope.resumed = draft ? { draft, state: "opening" } : null;
  scope.resumeOpened = () => Promise.resolve();
  scope.resumeFailed = () => Promise.resolve(null);
}

/** The draft still waiting in `scope` for its form. */
export function peekResumedDraft(scope: SessionScope = DEFAULT_SCOPE): Draft | null {
  return scope.resumed?.draft ?? null;
}
