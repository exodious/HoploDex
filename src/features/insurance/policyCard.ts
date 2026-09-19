/** DOM id of a policy's card on the Insurance page, so a link from another
 * page can scroll straight to it. */
export function policyCardId(policyId: number): string {
  return `policy-card-${policyId}`;
}
