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
/** Resolve global values and cross-Cube links through one deterministic policy. */
import { acceptsFieldValue, commonFieldDomain, initialCommonValue } from './FieldCompatibility.js';
import type {
  FieldContract,
  FieldDomain,
  FieldValue,
  NodeContract,
  OverrideGroup,
  RelationState,
  ResolvedField,
} from './RelationContracts.js';

const defaults = new Set(['encode_style', 'sampler_name', 'scheduler', 'seed']);
const labels: Readonly<Record<string, string>> = {
  encode_style: 'Encode Style',
  sampler_name: 'Sampler',
  scheduler: 'Scheduler',
  steps: 'Steps',
  cfg: 'CFG',
  seed: 'Seed',
};
const order = ['encode_style', 'sampler_name', 'scheduler', 'steps', 'cfg', 'seed'];

/** Discover compatible groups automatically without selected-value membership or manual matching. */
export function discoverOverrides(
  fields: readonly FieldContract[],
  state: RelationState,
): OverrideGroup[] {
  const buckets = new Map<string, FieldContract[][]>();
  for (const field of fields) {
    const groups = buckets.get(field.semantic) ?? [];
    const compatible = groups.find((group) => commonFieldDomain([...group, field]) !== null);
    if (compatible) compatible.push(field);
    else groups.push([field]);
    buckets.set(field.semantic, groups);
  }
  const result: OverrideGroup[] = [];
  for (const [semantic, groups] of buckets)
    for (const [index, members] of groups.entries()) {
      if (!labels[semantic]) continue;
      const domain = commonFieldDomain(members),
        first = members[0];
      if (!domain || !first) continue;
      const key = index === 0 ? semantic : `${semantic}:${first.id}`;
      const persisted = state.overrides[key];
      const value = initialCommonValue(domain, persisted?.value ?? first.value);
      result.push({
        key,
        label: `${labels[semantic] ?? humanName(semantic)}${index ? ` (${index + 1})` : ''}`,
        members,
        domain,
        enabled: persisted?.enabled ?? defaults.has(semantic),
        value,
        seedMode: persisted?.seedMode ?? first.seedMode ?? 'fixed',
      });
    }
  return result.sort(
    (a, b) =>
      (order.indexOf(a.members[0]?.semantic ?? '') + 1 || 100) -
        (order.indexOf(b.members[0]?.semantic ?? '') + 1 || 100) || a.key.localeCompare(b.key),
  );
}

/** Require the same registered type and complete compatible locally owned configuration. */
export function compatibleNodes(left: NodeContract, right: NodeContract): boolean {
  return (
    left.linkable &&
    right.linkable &&
    left.cubeId !== right.cubeId &&
    left.classType === right.classType &&
    left.configurationIdentity === right.configurationIdentity &&
    left.promptRole === right.promptRole &&
    left.fields.length > 0 &&
    left.fields.length === right.fields.length &&
    left.fields.every((field) => {
      const peer = right.fields.find((other) => other.name === field.name);
      return peer !== undefined && commonFieldDomain([field, peer]) !== null;
    })
  );
}

/** Select a compatible master strictly before its follower in the workflow order. */
export function canLinkNode(
  target: NodeContract,
  source: NodeContract,
  nodes: readonly NodeContract[],
  state: RelationState,
): boolean {
  return (
    nodes.indexOf(source) < nodes.indexOf(target) &&
    canShareNodeConfiguration(target, source, nodes, state)
  );
}

/** Validate a stored sharing group independently of the order being reconciled. */
export function canShareNodeConfiguration(
  target: NodeContract,
  source: NodeContract,
  nodes: readonly NodeContract[],
  state: RelationState,
): boolean {
  if (
    !compatibleNodes(target, source) ||
    !uniqueEndpoint(target, nodes) ||
    !uniqueEndpoint(source, nodes)
  )
    return false;
  const visited = new Set([target.id]);
  let anchor: string | null | undefined = source.id;
  while (anchor) {
    if (visited.has(anchor)) return false;
    visited.add(anchor);
    anchor = state.nodeLinks[anchor];
  }
  const next = { ...state, nodeLinks: { ...state.nodeLinks, [target.id]: source.id } };
  const fields = nodes.flatMap((node) => node.fields);
  return target.fields.every(
    (field) => commonFieldDomain(linkedFieldMembers(field.id, fields, nodes, next)) !== null,
  );
}

/** Reject indistinguishable endpoints inside one Cube rather than selecting an arbitrary node. */
function uniqueEndpoint(node: NodeContract, nodes: readonly NodeContract[]): boolean {
  return (
    nodes.filter(
      (peer) =>
        peer.linkable &&
        peer.cubeId === node.cubeId &&
        peer.classType === node.classType &&
        peer.configurationIdentity === node.configurationIdentity &&
        peer.promptRole === node.promptRole &&
        peer.fields.length === node.fields.length &&
        peer.fields.every((field) =>
          node.fields.some((candidate) => candidate.name === field.name),
        ),
    ).length === 1
  );
}

/** Retain explicit independence and select the earliest compatible Cube as the initial master. */
export function initializeNodeLinks(nodes: readonly NodeContract[], state: RelationState): void {
  for (const [index, node] of nodes.entries()) {
    if (Object.hasOwn(state.nodeLinks, node.id)) continue;
    const master = nodes.slice(0, index).find((peer) => canLinkNode(node, peer, nodes, state));
    if (master) {
      state.nodeLinks[node.id] = master.id;
      state.nodeLinks[master.id] ??= null;
    }
  }
}

/** Expand whole-node relationships into eligible field edges while retaining graph connections. */
export function relationEdges(
  fields: readonly FieldContract[],
  nodes: readonly NodeContract[],
  state: RelationState,
): Map<string, string> {
  const byId = new Map(fields.map((field) => [field.id, field]));
  const edges = new Map<string, string>();
  for (const node of nodes) {
    const sourceId = state.nodeLinks[node.id];
    const source = nodes.find((peer) => peer.id === sourceId);
    if (!source || !compatibleNodes(node, source)) continue;
    for (const field of node.fields) {
      const peer = source.fields.find((candidate) => candidate.name === field.name);
      if (peer) edges.set(field.id, peer.id);
    }
  }
  for (const [targetId, sourceId] of Object.entries(state.fieldLinks)) {
    if (!sourceId) continue;
    const target = byId.get(targetId),
      source = byId.get(sourceId);
    if (
      target &&
      source &&
      target.linkable !== false &&
      source.linkable !== false &&
      target.cubeId !== null &&
      source.cubeId !== null &&
      target.cubeId !== source.cubeId &&
      commonFieldDomain([target, source])
    )
      edges.set(targetId, sourceId);
  }
  return edges;
}

/** Resolve cycles as independent and validate the entire linked group before applying one value. */
export function resolveRelations(
  fields: readonly FieldContract[],
  nodes: readonly NodeContract[],
  state: RelationState,
  groups: readonly OverrideGroup[] = discoverOverrides(fields, state),
): ResolvedField[] {
  const byId = new Map(fields.map((field) => [field.id, field]));
  const edges = relationEdges(fields, nodes, state);
  const overrides = new Map(
    groups
      .filter((group) => group.enabled)
      .flatMap((group) => group.members.map((field) => [field.id, group] as const)),
  );
  const master = (id: string): string | null => {
    const visited = new Set<string>();
    while (edges.has(id)) {
      if (visited.has(id)) return null;
      visited.add(id);
      id = edges.get(id) ?? id;
    }
    return id;
  };
  const read = (field: FieldContract): FieldValue => state.locals[field.id] ?? field.value;
  const linkedGroups = new Map<string, FieldContract[]>();
  for (const field of fields) {
    const anchor = master(field.id);
    if (anchor) linkedGroups.set(anchor, [...(linkedGroups.get(anchor) ?? []), field]);
  }
  return fields.map((field) => {
    const override = overrides.get(field.id);
    if (override) return { field, value: override.value, source: null, override: override.key };
    const anchor = master(field.id),
      source = anchor ? byId.get(anchor) : undefined;
    const members = anchor ? (linkedGroups.get(anchor) ?? []) : [];
    const domain = commonFieldDomain(members);
    if (source && source.id !== field.id && domain) {
      const sourceValue = overrides.get(source.id)?.value ?? read(source);
      return {
        field,
        value: initialCommonValue(domain, sourceValue),
        source: source.id,
        override: null,
      };
    }
    const constrained =
      members.length > 1 && domain ? initialCommonValue(domain, read(field)) : read(field);
    return { field, value: constrained, source: null, override: null };
  });
}

/** Return every linked member whose domain restricts editing this field's master. */
export function linkedFieldMembers(
  fieldId: string,
  fields: readonly FieldContract[],
  nodes: readonly NodeContract[],
  state: RelationState,
): FieldContract[] {
  const edges = relationEdges(fields, nodes, state),
    ids = new Set([fieldId]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const [target, source] of edges)
      if (ids.has(target) || ids.has(source)) {
        const before = ids.size;
        ids.add(target);
        ids.add(source);
        changed ||= ids.size !== before;
      }
  }
  return fields.filter((field) => ids.has(field.id));
}

/** Validate a proposed link against its complete future group and reject directed cycles. */
export function canLinkField(
  target: FieldContract,
  source: FieldContract,
  fields: readonly FieldContract[],
  nodes: readonly NodeContract[],
  state: RelationState,
): boolean {
  if (target.linkable === false || source.linkable === false) return false;
  if (
    !target.cubeId ||
    !source.cubeId ||
    target.cubeId === source.cubeId ||
    !commonFieldDomain([target, source])
  )
    return false;
  const next = { ...state, fieldLinks: { ...state.fieldLinks, [target.id]: source.id } };
  const edges = relationEdges(fields, nodes, next),
    visited = new Set([target.id]);
  let id: string | undefined = source.id;
  while (id) {
    if (visited.has(id)) return false;
    visited.add(id);
    id = edges.get(id);
  }
  return commonFieldDomain(linkedFieldMembers(target.id, fields, nodes, next)) !== null;
}

/** Reject an invalid master edit rather than producing divergent clamped member values. */
export function validateSharedEdit(domain: FieldDomain | null, value: FieldValue): void {
  if (!acceptsFieldValue(domain, value))
    throw new RangeError('This value is unavailable to one or more linked fields.');
}

/** Format inferred field meaning using ordinary readable labels. */
function humanName(name: string): string {
  return name
    .split('_')
    .map((word) => (word[0]?.toUpperCase() ?? '') + word.slice(1))
    .join(' ');
}
