/** A hoplon — the round shield the name comes from — crossed by the same
 * dash-dot axis as the type drawings. */
export function BrandMark() {
  return (
    <svg className="hd-brand__mark" viewBox="0 0 28 28" aria-hidden focusable={false}>
      <circle cx="14" cy="14" r="11.5" />
      <circle cx="14" cy="14" r="7" />
      <path d="M1 14h26" className="hd-brand__axis" />
    </svg>
  );
}
