/** A firearm's name as rich content: make and model, with its nickname
 * (FR-031) set apart in quotes. Reads as the same text as `firearmName`. */
export function FirearmName({
  firearm,
}: {
  firearm: { make: string; model: string; nickname?: string | null };
}) {
  return (
    <>
      {firearm.make} {firearm.model}
      {firearm.nickname && (
        <>
          {" "}
          <span className="hd-nickname">“{firearm.nickname}”</span>
        </>
      )}
    </>
  );
}
