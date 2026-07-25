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
/** Extract persisted group-era Cubes before Comfy configures their runtime objects. */

import { isRecord } from '../../types/common.js';
import type { UnknownRecord, Vec2 } from '../../types/common.js';
import type { GraphId } from '../../types/graph.js';

export interface LegacyCubeLink {
  id: GraphId;
  originNodeId: GraphId;
  originSlot: number;
  targetNodeId: GraphId;
  targetSlot: number;
  type: string;
}

export interface LegacyCubeInputTarget {
  linkId: GraphId;
  nodeId: GraphId;
  slot: number;
}

export interface LegacyCubeInputPlan {
  markerId: GraphId;
  name: string;
  type: string;
  targets: LegacyCubeInputTarget[];
}

export interface LegacyCubeOutputPlan {
  markerId: GraphId;
  name: string;
  type: string;
  source: {
    linkId: GraphId;
    nodeId: GraphId;
    slot: number;
  };
}

export interface LegacyCubePlan {
  key: string;
  cubeId: string;
  cubeVersion: string;
  title: string;
  position: Vec2;
  size: Vec2;
  nodes: UnknownRecord[];
  groups: UnknownRecord[];
  internalLinks: LegacyCubeLink[];
  inputs: LegacyCubeInputPlan[];
  outputs: LegacyCubeOutputPlan[];
}

export type LegacyConnectionOrigin =
  | { kind: 'root'; nodeId: GraphId; slot: number }
  | { kind: 'cube-output'; cubeKey: string; name: string };

export type LegacyConnectionTarget =
  | { kind: 'root'; nodeId: GraphId; slot: number }
  | { kind: 'cube-input'; cubeKey: string; name: string };

export interface LegacyRootConnection {
  origin: LegacyConnectionOrigin;
  target: LegacyConnectionTarget;
  type: string;
}

export interface LegacyCubeMigrationBatch {
  plans: LegacyCubePlan[];
  connections: LegacyRootConnection[];
  warnings: string[];
}

interface ManagedGroup {
  source: UnknownRecord;
  plan: LegacyCubePlan;
  memberIds: Set<string>;
  inputMarkerIds: Set<string>;
  outputMarkerIds: Set<string>;
  bounds: [number, number, number, number];
}

interface MarkerEndpoint {
  cubeKey: string;
  name: string;
}

/** Own conversion and removal of persisted group-era workflow records. */
export class LegacyCubeWorkflowExtractor {
  /**
   * Extract legacy Cubes and remove every owned record from workflow configuration.
   *
   * The returned plans remain detached from the workflow arrays so Comfy never
   * constructs the obsolete marker nodes or managed groups.
   */
  extractInPlace(workflow: unknown): LegacyCubeMigrationBatch {
    if (!isRecord(workflow)) return emptyBatch();
    const nodes = readRecords(workflow.nodes);
    const groups = readRecords(workflow.groups);
    const rawLinks = Array.isArray(workflow.links) ? workflow.links : [];
    const nodeById = indexRecordsById(nodes);
    const links = rawLinks
      .map(parseLegacyLink)
      .filter((link): link is LegacyCubeLink => link !== null);
    const warnings: string[] = [];
    const managed = groups
      .map((group, index) =>
        this.#readManagedGroup(group, nodeById, warnings, `legacy-group-${index + 1}`),
      )
      .filter((group): group is ManagedGroup => group !== null);
    if (managed.length === 0) return emptyBatch();

    const extractedInnerGroups = this.#assignInnerGroups(managed, groups);
    const inputMarkers = new Map<string, MarkerEndpoint>();
    const outputMarkers = new Map<string, MarkerEndpoint>();
    const removedIds = new Set<string>();
    const consumedLinkIds = new Set<string>();

    for (const group of managed) {
      for (const id of group.memberIds) removedIds.add(id);
      for (const id of group.inputMarkerIds) {
        removedIds.add(id);
        const marker = nodeById.get(id);
        inputMarkers.set(id, {
          cubeKey: group.plan.key,
          name: readMarkerName(marker, `input.${id}`),
        });
      }
      for (const id of group.outputMarkerIds) {
        removedIds.add(id);
        const marker = nodeById.get(id);
        outputMarkers.set(id, {
          cubeKey: group.plan.key,
          name: readMarkerName(marker, `output.${id}`),
        });
      }
      this.#collectTopology(group, links, nodeById, consumedLinkIds, warnings);
    }

    const connections = this.#collectExternalConnections(
      links,
      removedIds,
      consumedLinkIds,
      inputMarkers,
      outputMarkers,
      warnings,
    );
    const extractedGroups = new Set<UnknownRecord>(managed.map((group) => group.source));
    for (const inner of extractedInnerGroups) extractedGroups.add(inner);

    workflow.nodes = nodes.filter((node) => !removedIds.has(readIdKey(node.id)));
    workflow.links = rawLinks.filter((rawLink) => {
      const link = parseLegacyLink(rawLink);
      return (
        link === null ||
        (!removedIds.has(readIdKey(link.originNodeId)) &&
          !removedIds.has(readIdKey(link.targetNodeId)))
      );
    });
    workflow.groups = groups.filter((group) => !extractedGroups.has(group));
    return {
      plans: managed.map((group) => group.plan),
      connections,
      warnings,
    };
  }

  /** Parse one authoritative managed-group record and its owned node identities. */
  #readManagedGroup(
    group: UnknownRecord,
    nodeById: ReadonlyMap<string, UnknownRecord>,
    warnings: string[],
    fallbackKey: string,
  ): ManagedGroup | null {
    const metadata = isRecord(group.sugarcubes) ? group.sugarcubes : null;
    if (!metadata || metadata.managed === false) return null;
    const memberIds = readIdSet(metadata.nodes);
    const markers = isRecord(metadata.markers) ? metadata.markers : {};
    const inputMarkerIds = readIdSet(markers.inputs);
    const outputMarkerIds = readIdSet(markers.outputs);
    if (memberIds.size === 0 && inputMarkerIds.size === 0 && outputMarkerIds.size === 0) {
      return null;
    }
    const bounds = readBounds(group.bounding);
    const groupId = readIdKey(group.id);
    const key =
      readString(metadata.instance_id) || (groupId ? `legacy-group-${groupId}` : fallbackKey);
    const title =
      readString(metadata.alias) ||
      readString(metadata.cube_name) ||
      readString(group.title) ||
      'SugarCube';
    const ownedNodes: UnknownRecord[] = [];
    for (const id of memberIds) {
      const node = nodeById.get(id);
      if (node) ownedNodes.push(cloneRecord(node));
      else warnings.push(`Legacy Cube '${title}' references missing node '${id}'.`);
    }
    return {
      source: group,
      memberIds,
      inputMarkerIds,
      outputMarkerIds,
      bounds,
      plan: {
        key,
        cubeId: readString(metadata.cube_id),
        cubeVersion: readString(metadata.cube_version),
        title,
        position: [bounds[0], bounds[1]],
        size: [bounds[2], bounds[3]],
        nodes: ownedNodes,
        groups: [],
        internalLinks: [],
        inputs: [],
        outputs: [],
      },
    };
  }

  /** Assign each plain nested layout group to the smallest containing Cube. */
  #assignInnerGroups(managed: ManagedGroup[], groups: UnknownRecord[]): Set<UnknownRecord> {
    const extracted = new Set<UnknownRecord>();
    for (const candidate of groups) {
      if (isRecord(candidate.sugarcubes)) continue;
      const bounds = readBounds(candidate.bounding);
      const owners = managed
        .filter((group) => containsBounds(group.bounds, bounds))
        .sort((left, right) => area(left.bounds) - area(right.bounds));
      const owner = owners[0];
      if (owner) {
        owner.plan.groups.push(cloneRecord(candidate));
        extracted.add(candidate);
      }
    }
    return extracted;
  }

  /** Collect native internal links and marker-to-boundary topology for one Cube. */
  #collectTopology(
    group: ManagedGroup,
    links: LegacyCubeLink[],
    nodeById: ReadonlyMap<string, UnknownRecord>,
    consumedLinkIds: Set<string>,
    warnings: string[],
  ): void {
    const inputTargets = new Map<string, LegacyCubeInputTarget[]>();
    const outputSources = new Map<string, LegacyCubeLink[]>();
    for (const link of links) {
      const origin = readIdKey(link.originNodeId);
      const target = readIdKey(link.targetNodeId);
      if (group.memberIds.has(origin) && group.memberIds.has(target)) {
        group.plan.internalLinks.push(link);
        consumedLinkIds.add(readIdKey(link.id));
      } else if (group.inputMarkerIds.has(origin) && group.memberIds.has(target)) {
        const targets = inputTargets.get(origin) ?? [];
        targets.push({
          linkId: link.id,
          nodeId: link.targetNodeId,
          slot: link.targetSlot,
        });
        inputTargets.set(origin, targets);
        consumedLinkIds.add(readIdKey(link.id));
      } else if (group.memberIds.has(origin) && group.outputMarkerIds.has(target)) {
        const sources = outputSources.get(target) ?? [];
        sources.push(link);
        outputSources.set(target, sources);
        consumedLinkIds.add(readIdKey(link.id));
      }
    }

    for (const markerId of group.inputMarkerIds) {
      const marker = nodeById.get(markerId);
      const targets = inputTargets.get(markerId) ?? [];
      if (targets.length === 0) {
        warnings.push(
          `Legacy Cube '${group.plan.title}' input '${readMarkerName(marker, markerId)}' has no target.`,
        );
        continue;
      }
      group.plan.inputs.push({
        markerId: readGraphId(marker?.id, markerId),
        name: readMarkerName(marker, `input.${markerId}`),
        type: readMarkerType(marker, 'output', targets[0]?.linkId, links),
        targets,
      });
    }
    for (const markerId of group.outputMarkerIds) {
      const marker = nodeById.get(markerId);
      const sources = outputSources.get(markerId) ?? [];
      const source = sources[0];
      if (!source) {
        warnings.push(
          `Legacy Cube '${group.plan.title}' output '${readMarkerName(marker, markerId)}' has no source.`,
        );
        continue;
      }
      if (sources.length > 1) {
        warnings.push(
          `Legacy Cube '${group.plan.title}' output '${readMarkerName(marker, markerId)}' had multiple sources; the first was preserved.`,
        );
      }
      group.plan.outputs.push({
        markerId: readGraphId(marker?.id, markerId),
        name: readMarkerName(marker, `output.${markerId}`),
        type: readMarkerType(marker, 'input', source.id, links),
        source: {
          linkId: source.id,
          nodeId: source.originNodeId,
          slot: source.originSlot,
        },
      });
    }
  }

  /** Convert every non-internal legacy link through a semantic root endpoint. */
  #collectExternalConnections(
    links: LegacyCubeLink[],
    removedIds: ReadonlySet<string>,
    consumedLinkIds: ReadonlySet<string>,
    inputMarkers: ReadonlyMap<string, MarkerEndpoint>,
    outputMarkers: ReadonlyMap<string, MarkerEndpoint>,
    warnings: string[],
  ): LegacyRootConnection[] {
    const connections: LegacyRootConnection[] = [];
    for (const link of links) {
      if (consumedLinkIds.has(readIdKey(link.id))) continue;
      const originId = readIdKey(link.originNodeId);
      const targetId = readIdKey(link.targetNodeId);
      if (!removedIds.has(originId) && !removedIds.has(targetId)) continue;
      const markerOrigin = outputMarkers.get(originId);
      const markerTarget = inputMarkers.get(targetId);
      const origin: LegacyConnectionOrigin | null = markerOrigin
        ? {
            kind: 'cube-output',
            cubeKey: markerOrigin.cubeKey,
            name: markerOrigin.name,
          }
        : !removedIds.has(originId)
          ? { kind: 'root', nodeId: link.originNodeId, slot: link.originSlot }
          : null;
      const target: LegacyConnectionTarget | null = markerTarget
        ? {
            kind: 'cube-input',
            cubeKey: markerTarget.cubeKey,
            name: markerTarget.name,
          }
        : !removedIds.has(targetId)
          ? { kind: 'root', nodeId: link.targetNodeId, slot: link.targetSlot }
          : null;
      if (origin && target) {
        connections.push({ origin, target, type: link.type });
      } else {
        warnings.push(
          `Legacy link '${String(link.id)}' bypasses Cube boundary markers and could not be migrated.`,
        );
      }
    }
    return connections;
  }
}

/** Return a detached empty migration result. */
function emptyBatch(): LegacyCubeMigrationBatch {
  return { plans: [], connections: [], warnings: [] };
}

/** Parse one legacy array or object link shape. */
function parseLegacyLink(value: unknown): LegacyCubeLink | null {
  const tuple = Array.isArray(value) ? value : null;
  const record = isRecord(value) ? value : null;
  const id = tuple?.[0] ?? record?.id;
  const originNodeId = tuple?.[1] ?? record?.origin_id;
  const originSlot = readSlot(tuple?.[2] ?? record?.origin_slot);
  const targetNodeId = tuple?.[3] ?? record?.target_id;
  const targetSlot = readSlot(tuple?.[4] ?? record?.target_slot);
  if (
    !isGraphId(id) ||
    !isGraphId(originNodeId) ||
    !isGraphId(targetNodeId) ||
    originSlot === null ||
    targetSlot === null
  ) {
    return null;
  }
  return {
    id,
    originNodeId,
    originSlot,
    targetNodeId,
    targetSlot,
    type: readString(tuple?.[5] ?? record?.type) || '*',
  };
}

/** Read a non-negative integer slot index. */
function readSlot(value: unknown): number | null {
  const slot = Number(value);
  return Number.isInteger(slot) && slot >= 0 ? slot : null;
}

/** Identify IDs accepted by LiteGraph serialization. */
function isGraphId(value: unknown): value is GraphId {
  return (
    (typeof value === 'string' && value.length > 0) ||
    (typeof value === 'number' && Number.isFinite(value))
  );
}

/** Read an ID or retain a stable fallback. */
function readGraphId(value: unknown, fallback: GraphId): GraphId {
  return isGraphId(value) ? value : fallback;
}

/** Normalize one graph identity for set membership. */
function readIdKey(value: unknown): string {
  return isGraphId(value) ? String(value) : '';
}

/** Read a set of serialized graph identities. */
function readIdSet(value: unknown): Set<string> {
  return new Set(
    (Array.isArray(value) ? value : []).map(readIdKey).filter((entry) => entry.length > 0),
  );
}

/** Index serialized records by graph ID. */
function indexRecordsById(records: UnknownRecord[]): Map<string, UnknownRecord> {
  const index = new Map<string, UnknownRecord>();
  for (const record of records) {
    const key = readIdKey(record.id);
    if (key) index.set(key, record);
  }
  return index;
}

/** Return only record entries from an untrusted workflow collection. */
function readRecords(value: unknown): UnknownRecord[] {
  return Array.isArray(value) ? value.filter(isRecord) : [];
}

/** Read one marker's stable symbolic boundary name. */
function readMarkerName(marker: UnknownRecord | undefined, fallback: string): string {
  const properties = isRecord(marker?.properties) ? marker.properties : {};
  return readString(properties.sugarcubes_symbol) || fallback;
}

/** Read one marker slot type, falling back to its internal link type. */
function readMarkerType(
  marker: UnknownRecord | undefined,
  side: 'input' | 'output',
  linkId: GraphId | undefined,
  links: LegacyCubeLink[],
): string {
  const slots = side === 'input' ? marker?.inputs : marker?.outputs;
  const first = Array.isArray(slots) && isRecord(slots[0]) ? slots[0] : null;
  return (
    readString(first?.type) ||
    links.find((link) => readIdKey(link.id) === readIdKey(linkId))?.type ||
    '*'
  );
}

/** Read finite group bounds with a safe minimum parent size. */
function readBounds(value: unknown): [number, number, number, number] {
  if (!Array.isArray(value)) return [0, 0, 900, 600];
  const x = Number(value[0]);
  const y = Number(value[1]);
  const width = Number(value[2]);
  const height = Number(value[3]);
  return [
    Number.isFinite(x) ? x : 0,
    Number.isFinite(y) ? y : 0,
    Number.isFinite(width) && width > 0 ? width : 900,
    Number.isFinite(height) && height > 0 ? height : 600,
  ];
}

/** Test whether one serialized bounding box is wholly within another. */
function containsBounds(
  outer: [number, number, number, number],
  inner: [number, number, number, number],
): boolean {
  return (
    inner[0] >= outer[0] &&
    inner[1] >= outer[1] &&
    inner[0] + inner[2] <= outer[0] + outer[2] &&
    inner[1] + inner[3] <= outer[1] + outer[3]
  );
}

/** Compute a serialized bounding-box area. */
function area(bounds: [number, number, number, number]): number {
  return bounds[2] * bounds[3];
}

/** Clone JSON-safe persisted workflow data before Comfy mutates it. */
function cloneRecord(value: UnknownRecord): UnknownRecord {
  const parsed: unknown = JSON.parse(JSON.stringify(value));
  return isRecord(parsed) ? parsed : {};
}

/** Read one trimmed persisted string. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
