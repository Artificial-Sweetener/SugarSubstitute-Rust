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
/** Reconcile durable sharing intentions with the workflow's current editable capabilities. */
import type { FieldContract, NodeContract, RelationState } from './RelationContracts.js';
import {
  canLinkField,
  canShareNodeConfiguration,
  initializeNodeLinks,
  resolveRelations,
} from './RelationResolver.js';

/** Preserve valid relations, release invalid ones and promote removed masters in display order. */
export function reconcileRelationMembership(
  fields: readonly FieldContract[],
  nodes: readonly NodeContract[],
  state: RelationState,
): void {
  const nodeIds = new Set(nodes.map((node) => node.id));
  const fieldIds = new Set(fields.map((field) => field.id));
  promoteMissingMasters(
    state.nodeLinks,
    nodes.map((node) => node.id),
    (id) => nodes.some((node) => node.id === id && node.linkable && node.fields.length > 0),
    (id) => {
      for (const field of fields) if (field.nodeId === id) state.locals[field.id] = field.value;
      state.localNodeModes[id] = nodes.find((node) => node.id === id)?.mode ?? 0;
    },
  );
  promoteMissingMasters(
    state.fieldLinks,
    fields.map((field) => field.id),
    (id) =>
      fields.some((field) => field.id === id && field.linkable !== false && field.cubeId !== null),
    (id) => {
      const field = fields.find((field) => field.id === id);
      if (field) state.locals[id] = field.value;
    },
  );
  // Validate the same snapshot before clearing any edge, so cycles release every participant.
  const invalidNodes = nodes.filter((node) => {
    const sourceId = state.nodeLinks[node.id];
    if (!sourceId) return false;
    const source = nodes.find((candidate) => candidate.id === sourceId);
    return !source || !canShareNodeConfiguration(node, source, nodes, state);
  });
  for (const node of invalidNodes) state.nodeLinks[node.id] = null;
  const invalidFields = fields.filter((field) => {
    const sourceId = state.fieldLinks[field.id];
    if (!sourceId) return false;
    const source = fields.find((candidate) => candidate.id === sourceId);
    return !source || !canLinkField(field, source, fields, nodes, state);
  });
  for (const field of invalidFields) state.fieldLinks[field.id] = null;
  pruneMissing(state.nodeLinks, nodeIds);
  pruneMissing(state.localNodeModes, nodeIds);
  pruneMissing(state.fieldLinks, fieldIds);
  pruneMissing(state.locals, fieldIds);
  pruneMissing(state.localSeedModes, fieldIds);
  normalizeNodeMasters(fields, nodes, state);
  initializeNodeLinks(nodes, state);
}

/** Promote the first surviving group member with shared contents and direct every follower to it. */
function normalizeNodeMasters(
  fields: readonly FieldContract[],
  nodes: readonly NodeContract[],
  state: RelationState,
): void {
  const groups = new Map<string, NodeContract[]>();
  const resolved = resolveRelations(fields, nodes, state, []);
  for (const node of nodes) {
    let anchor = node.id;
    const visited = new Set<string>();
    while (state.nodeLinks[anchor] && !visited.has(anchor)) {
      visited.add(anchor);
      anchor = state.nodeLinks[anchor] ?? anchor;
    }
    groups.set(anchor, [...(groups.get(anchor) ?? []), node]);
  }
  for (const [anchor, members] of groups) {
    const first = members[0];
    if (!first || members.length < 2) continue;
    if (first.id !== anchor) {
      for (const field of first.fields) {
        const effective = resolved.find((item) => item.field.id === field.id);
        if (effective) state.locals[field.id] = effective.value;
      }
      const source = nodes.find((node) => node.id === anchor);
      state.localNodeModes[first.id] = state.localNodeModes[anchor] ?? source?.mode ?? 0;
    }
    state.nodeLinks[first.id] = null;
    for (const member of members.slice(1)) state.nodeLinks[member.id] = first.id;
  }
}

/** Preserve effective configuration when an absent anchor's surviving group needs a new owner. */
function promoteMissingMasters(
  links: Record<string, string | null>,
  order: readonly string[],
  eligible: (id: string) => boolean,
  promote: (id: string) => void,
): void {
  const ids = new Set(order);
  const groups = new Map<string, string[]>();
  for (const id of order) {
    const anchor = missingAnchor(id, links, ids);
    if (anchor) groups.set(anchor, [...(groups.get(anchor) ?? []), id]);
  }
  for (const members of groups.values()) {
    const replacement = members.find(eligible);
    if (replacement) {
      links[replacement] = null;
      promote(replacement);
    }
    for (const id of members)
      if (id !== replacement) links[id] = eligible(id) ? (replacement ?? null) : null;
  }
}

/** Trace missing anchors through removed intermediate followers without treating cycles as groups. */
function missingAnchor(
  id: string,
  links: Readonly<Record<string, string | null>>,
  ids: ReadonlySet<string>,
): string | null {
  const visited = new Set<string>();
  let source = links[id];
  while (source) {
    if (visited.has(id)) return null;
    visited.add(id);
    id = source;
    source = links[id];
  }
  return ids.has(id) ? null : id;
}

/** Remove retired stable identities without discarding dormant values on surviving capabilities. */
function pruneMissing<T>(values: Record<string, T>, ids: ReadonlySet<string>): void {
  for (const id of Object.keys(values)) if (!ids.has(id)) Reflect.deleteProperty(values, id);
}
