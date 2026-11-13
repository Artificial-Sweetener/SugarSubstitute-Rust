//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Canonical layout observation has a separate finite budget from copied pixel snapshots. */
import { requireValue } from '../../core/src/required.js';
import type { MountedReorderTarget } from './reorder-mounted-target.js';
/** One complete observation shares native placement limits across every disjoint target subtree. */
interface LayoutBudget {
  nodes: number;
  characters: number;
}
/** One native read's copied topology is reusable only under the caller's exact mutation-observed issue. */
interface ObservedNode {
  readonly length: number;
  readonly children: readonly Node[];
}
/** Complete target and line outputs retain separate occurrence budgets. */
export interface ReorderLayoutAdmission {
  readonly targets: ReadonlyMap<HTMLElement, readonly Node[]>;
  readonly nodes: readonly Node[];
}
/** Traverse actual nodes without reading text values or allocating paint. */
function admitNodes(
  roots: readonly Node[],
  budget: LayoutBudget,
  observed?: Map<Node, ObservedNode>,
  reuse = false,
): readonly Node[] | null {
  const nodes: Node[] = [],
    pending: Node[] = [...roots];
  while (pending.length) {
    const node = requireValue(pending.pop(), 'reorder-layout-admission');
    budget.nodes++;
    const retained = reuse ? observed?.get(node) : undefined;
    const length = retained?.length ?? (node.nodeType === 3 ? (node as Text).length : 0);
    if (!Number.isSafeInteger(length) || length < 0) return null;
    budget.characters += length;
    if (budget.nodes > 8192 || budget.characters > 16384) return null;
    const children = retained?.children ?? node.childNodes,
      childCount = children.length;
    if (pending.length + childCount + budget.nodes > 8192) return null;
    nodes.push(node);
    const copied = observed && !retained ? new Array<Node>(childCount) : null;
    for (let at = childCount - 1; at >= 0; at--) {
      const child = requireValue(children[at], 'reorder-layout-admission');
      pending.push(child);
      if (copied) copied[at] = child;
    }
    if (copied)
      requireValue(observed, 'reorder-layout-admission').set(node, { length, children: copied });
  }
  return nodes;
}
/** Disjoint semantic targets retain one node index under unchanged native placement bounds. */
function admitTargets(
  targets: readonly MountedReorderTarget[],
  observed?: Map<Node, ObservedNode>,
  reuse = false,
): ReadonlyMap<HTMLElement, readonly Node[]> | null {
  if (targets.length > 8192) return null;
  const result = new Map<HTMLElement, readonly Node[]>(),
    budget = { nodes: 0, characters: 0 };
  for (const target of targets) {
    let roots: readonly Node[];
    if (reuse) {
      const parent = target.kind === 'chip' ? null : target.node.parentNode;
      const carrier = parent ? observed?.get(parent)?.children[0] : null;
      if (target.kind !== 'chip' && !carrier) return null;
      roots =
        target.kind === 'chip'
          ? [target.node]
          : [target.node, requireValue(carrier, 'reorder-layout-admission')];
      if (roots.some((node) => !observed?.has(node))) return null;
    } else {
      roots =
        target.kind === 'chip'
          ? [target.node]
          : [
              target.node,
              requireValue(
                requireValue(target.node.parentNode, 'reorder-layout-admission').childNodes[0],
                'reorder-layout-admission',
              ),
            ];
    }
    const nodes = admitNodes(roots, budget, observed, reuse);
    if (!nodes) return null;
    result.set(target.node, nodes);
  }
  return result;
}
/** Standalone target admission always reads its complete actual native topology. */
export function admitReorderTargetLayout(
  targets: readonly MountedReorderTarget[],
): ReadonlyMap<HTMLElement, readonly Node[]> | null {
  return admitTargets(targets);
}
/** Reuse only one request's native operands, fenced across every delegated discovery and before publication. */
export function admitReorderLayout(
  targets: readonly MountedReorderTarget[],
  readLines: () => readonly HTMLElement[] | null,
  current: () => boolean,
): ReorderLayoutAdmission | null {
  const observed = new Map<Node, ObservedNode>(),
    admitted = admitTargets(targets, observed);
  if (!admitted || !current()) return null;
  const roots = readLines();
  if (!roots || !current()) return null;
  const nodes = admitNodes(roots, { nodes: 0, characters: 0 }, observed, true);
  return nodes && current() ? Object.freeze({ targets: admitted, nodes }) : null;
}
/** One synchronous copied-tree observation retains actual topology only until its request retires. */
export interface ReorderLineLayoutRead {
  readonly nodes: readonly Node[];
  /** Target occurrences retain their independent original node/character budgets. */
  targets(
    targets: readonly MountedReorderTarget[],
  ): ReadonlyMap<HTMLElement, readonly Node[]> | null;
  /** Release copied topology before native observer disposal, including failed or reentrant reads. */
  retire(): void;
}
/** Full-shell-first native discovery may serve later target admission only under an exact mutation-observed request. */
export function captureReorderLineLayout(
  roots: readonly HTMLElement[],
  current: () => boolean,
): ReorderLineLayoutRead | null {
  const observed = new Map<Node, ObservedNode>();
  const nodes = admitNodes(roots, { nodes: 0, characters: 0 }, observed);
  if (!nodes || !current()) {
    observed.clear();
    return null;
  }
  let active = true;
  return Object.freeze({
    nodes,
    targets: (targets: readonly MountedReorderTarget[]) => {
      if (!active || !current() || !active) return null;
      const admitted = admitTargets(targets, observed, true);
      return active && current() && active ? admitted : null;
    },
    retire: () => {
      active = false;
      observed.clear();
    },
  });
}
/** Full canonical carriers retain logical shaping while the same finite observation budget guards their mount. */
export function admitReorderLineLayout(roots: readonly HTMLElement[]): readonly Node[] | null {
  return admitNodes(roots, { nodes: 0, characters: 0 });
}
