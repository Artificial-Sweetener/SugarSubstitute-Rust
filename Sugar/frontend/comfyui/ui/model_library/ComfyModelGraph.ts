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
/** Resolve exact graph-node ownership independently of a node's temporary presentation. */
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ModelPickerBinding } from './ModelPickerBinding.js';

export interface GraphScope {
  readonly graph: UnknownRecord;
  readonly nodes: readonly UnknownRecord[];
  readonly owner: UnknownRecord | null;
}

/** Find exact native graph ownership while refusing ambiguous numeric identities. */
export function locateModelGraph(
  app: unknown,
  binding: ModelPickerBinding,
): { scope: GraphScope; node: UnknownRecord } | null {
  if (!isRecord(app)) return null;
  const locator =
    binding.anchor.closest<HTMLElement>('[data-cube-node-locator]')?.dataset.cubeNodeLocator ??
    binding.nodeLocator;
  const nodeId = binding.anchor.closest<HTMLElement>('[data-node-id]')?.dataset.nodeId;
  const matches: { scope: GraphScope; node: UnknownRecord }[] = [];
  for (const scope of graphScopes(app.graph, new Set()))
    for (const node of scope.nodes)
      if (
        locator
          ? `${String(scope.graph.id)}:${String(node.id)}` === locator
          : scope.graph === app.graph && nodeId === String(node.id)
      )
        matches.push({ scope, node });
  return matches.length === 1 ? (matches[0] ?? null) : null;
}

/** Collect native subgraphs once while bounding cyclic host structures. */
function graphScopes(
  value: unknown,
  seen: Set<object>,
  owner: UnknownRecord | null = null,
): GraphScope[] {
  if (!isRecord(value) || seen.has(value) || seen.size > 2048) return [];
  seen.add(value);
  const nodes = Array.isArray(value._nodes) ? value._nodes.filter(isRecord) : [];
  return [
    { graph: value, nodes, owner },
    ...nodes.flatMap((node) => graphScopes(node.subgraph, seen, node)),
    ...(value._subgraphs instanceof Map
      ? [...value._subgraphs.values()].flatMap((graph) => graphScopes(graph, seen))
      : []),
  ];
}

/** Locate the same node object after presentation remounts, rejecting replacement workflows. */
export function findModelGraphNode(app: unknown, node: object): GraphScope | null {
  if (!isRecord(app)) return null;
  return (
    graphScopes(app.graph, new Set()).find((scope) =>
      scope.nodes.some((candidate) => candidate === node),
    ) ?? null
  );
}
