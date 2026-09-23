import { useState } from "react";
import { Button, Icon } from "../../components";
import { BackLink } from "../app/BackLink";
import { useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import { RunningHead } from "../app/RunningHead";
import { PolicyCard } from "./PolicyCard";
import { usePolicyEditors } from "./PolicyEditors";
import "./insurance.css";

/** One policy on its own, reached from a firearm it covers. It is the same
 * card the Insurance page lists, with the same edit and delete actions,
 * which a pinned strip keeps in reach once the card's header scrolls away. */
export function PolicyPage({ id }: { id: number }) {
  const { firearms, summary, policiesById } = useCollection();
  const { navigate, back } = useNavigation();
  const { edit, remove, dialogs } = usePolicyEditors(() => navigate({ page: "insurance" }));
  const [head, setHead] = useState<HTMLElement | null>(null);
  const policy = policiesById.get(id);

  return (
    <>
      {back && (
        <div className="hd-page-back">
          <BackLink target={back} />
        </div>
      )}
      {policy ? (
        <>
          <PolicyCard
            policy={policy}
            blanket={summary?.blanket ?? null}
            summary={summary?.byPolicy.find((p) => p.policyId === policy.id)}
            firearms={firearms.filter(
              (f) => f.insurancePolicyId === policy.id && f.status === "active",
            )}
            onEdit={() => edit(policy)}
            onDelete={() => remove(policy)}
            headRef={setHead}
          />
          <RunningHead
            anchor={head}
            headingId={`policy-${policy.id}`}
            title={policy.name}
            stamp={policy.policyNumber}
            actions={
              <>
                <Button size="sm" icon="pencil" onClick={() => edit(policy)}>
                  Edit
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  icon="trash"
                  aria-label={`Delete ${policy.name}`}
                  onClick={() => remove(policy)}
                />
              </>
            }
          />
        </>
      ) : (
        <div className="hd-empty hd-empty--compact">
          <Icon name="shield" size={28} />
          <p className="hd-empty__text">This policy no longer exists.</p>
        </div>
      )}
      {dialogs}
    </>
  );
}
