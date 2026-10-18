// The SPDX expression evaluator (010 research.md §5): `satisfies()`, moved
// unchanged from check-npm-licenses.mjs, and the choice resolver `choose()`.

import { PREFERENCE } from "./policy.mjs";

/**
 * Parses an SPDX expression (OR, AND, WITH, parentheses) into a tree:
 * `{ id }`, `{ or: [...] }` or `{ and: [...] }`.
 */
function parse(expression) {
  const tokens = expression.match(/\(|\)|[^\s()]+/g) ?? [];
  let pos = 0;
  const peek = () => tokens[pos];
  function primary() {
    if (peek() === "(") {
      pos++;
      const value = or();
      if (tokens[pos++] !== ")") throw new Error("unbalanced parentheses");
      return value;
    }
    let id = tokens[pos++];
    if (id === undefined) throw new Error("unexpected end");
    if (peek() === "WITH") {
      pos++;
      id = `${id} WITH ${tokens[pos++]}`;
    }
    return { id };
  }
  function and() {
    const parts = [primary()];
    while (peek() === "AND") {
      pos++;
      parts.push(primary());
    }
    return parts.length === 1 ? parts[0] : { and: parts };
  }
  function or() {
    const parts = [and()];
    while (peek() === "OR") {
      pos++;
      parts.push(and());
    }
    return parts.length === 1 ? parts[0] : { or: parts };
  }
  const tree = or();
  if (pos !== tokens.length) throw new Error(`unexpected "${peek()}"`);
  return tree;
}

/**
 * Evaluates an SPDX expression (OR, AND, WITH, parentheses) against a
 * predicate for single license identifiers.
 */
export function satisfies(expression, isAllowed) {
  const evaluate = (node) => {
    if (node.id !== undefined) return isAllowed(node.id);
    if (node.and) return node.and.map(evaluate).every(Boolean);
    return node.or.map(evaluate).some(Boolean);
  };
  return evaluate(parse(expression));
}

const rank = (id) => {
  const index = PREFERENCE.indexOf(id);
  return index === -1 ? PREFERENCE.length : index;
};

/**
 * Resolves an expression to the licenses it is used under: a choice (`OR`)
 * becomes the first allowed alternative in PREFERENCE order, a conjunction
 * (`AND`) keeps every part, and a `WITH` exception stays whole. Returns the
 * licenses joined by ` AND `, or `undefined` when no choice is allowed.
 */
export function choose(expression, isAllowed) {
  const resolve = (node) => {
    // Each result is { ids, rank } where rank is the worst license in `ids`.
    if (node.id !== undefined) {
      return isAllowed(node.id) ? { ids: [node.id], rank: rank(node.id) } : undefined;
    }
    if (node.and) {
      const parts = node.and.map(resolve);
      if (parts.some((part) => part === undefined)) return undefined;
      return {
        ids: parts.flatMap((part) => part.ids),
        rank: Math.max(...parts.map((part) => part.rank)),
      };
    }
    let best;
    for (const part of node.or.map(resolve)) {
      if (part !== undefined && (best === undefined || part.rank < best.rank)) best = part;
    }
    return best;
  };
  return resolve(parse(expression))?.ids.join(" AND ");
}
