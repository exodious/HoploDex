import { useState } from "react";
import type { Dispatch, SetStateAction } from "react";

/** How many rows a list page has rendered so far (progressive rendering: more
 * mount as the end of the list nears), starting from `step` again whenever
 * `resetKey` changes: a new search, grouping or disposed filter.
 *
 * The reset happens in the render that sees the new key, not in an effect
 * after it. An effect would let that render draw every row mounted so far
 * (the old list, now under the new columns) before the next one cut it back
 * to `step` rows, which with thousands of rows cost seconds (issue #51). */
export function useRenderLimit(
  step: number,
  resetKey: string,
): [number, Dispatch<SetStateAction<number>>] {
  const [limit, setLimit] = useState(step);
  const [limitKey, setLimitKey] = useState(resetKey);
  if (limitKey !== resetKey) {
    setLimitKey(resetKey);
    setLimit(step);
  }
  return [limit, setLimit];
}
