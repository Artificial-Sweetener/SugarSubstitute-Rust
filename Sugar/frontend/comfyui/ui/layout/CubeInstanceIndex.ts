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
/**
 * Own the SugarCubes layout orchestration layer in `frontend/comfyui/ui/layout/CubeInstanceIndex.js`.
 */

import { resolveInstanceBounds } from '../graph/CubeBounds.js';
import { getGraphGroups } from '../graph/GraphQuery.js';
import { InstanceBuilder } from '../graph/InstanceBuilder.js';
import { getGroupSugarcubes } from '../graph/GroupMetadata.js';
import type { CubeGroupMetadataRecord } from '../graph/GroupMetadata.js';
import type { CubeInstance } from '../graph/InstanceBuilder.js';
import { isRecord } from '../types/common.js';
import type { RectBounds } from '../types/common.js';
import type { ComfyGraph, ComfyGroup, GraphId } from '../types/graph.js';

interface InstanceBuilderTarget {
  build(graph: ComfyGraph | null | undefined): CubeInstance[];
}

interface MatchedGroup {
  group: ComfyGroup;
  metadata: CubeGroupMetadataRecord;
}

interface GroupIndex {
  byInstanceId: Map<string, MatchedGroup>;
  byMarkerSignature: Map<string, MatchedGroup>;
}

export interface IndexedCubeInstance extends CubeInstance {
  group: ComfyGroup | null;
  metadata: CubeGroupMetadataRecord | null;
  bounds: RectBounds | null;
}

export interface CubeInstanceIndexResult {
  instances: IndexedCubeInstance[];
  instanceById: Map<string, IndexedCubeInstance>;
  instanceByCubeId: Map<string, IndexedCubeInstance>;
  instanceByMarkerId: Map<string, IndexedCubeInstance>;
  instanceByNodeId: Map<string, IndexedCubeInstance>;
}

interface BuildIndexOptions {
  graph: ComfyGraph | null;
  instanceBuilder: InstanceBuilderTarget | null;
  groups: ComfyGroup[] | null;
}

/**
 * Read marker ids from metadata.
 */
export function readMarkerIdsFromMetadata(
  metadata: CubeGroupMetadataRecord | null | undefined,
): string[] {
  if (!isRecord(metadata?.markers)) {
    return [];
  }
  const markers = metadata.markers;
  return [
    ...(Array.isArray(markers.inputs) ? markers.inputs : []),
    ...(Array.isArray(markers.outputs) ? markers.outputs : []),
  ];
}

/**
 * Build marker signature.
 */
export function buildMarkerSignature(markerIds: readonly GraphId[] | null | undefined): string {
  if (!Array.isArray(markerIds) || !markerIds.length) {
    return '';
  }
  return markerIds
    .map((value) => String(value))
    .sort()
    .join('|');
}

function buildGroupIndex(groups: readonly ComfyGroup[]): GroupIndex {
  const byInstanceId = new Map<string, MatchedGroup>();
  const byMarkerSignature = new Map<string, MatchedGroup>();
  for (const group of groups) {
    const metadata = getGroupSugarcubes(group);
    if (metadata?.instance_id) {
      byInstanceId.set(String(metadata.instance_id), { group, metadata });
    }
    if (metadata?.managed) {
      const signatureKey = buildMarkerSignature(readMarkerIdsFromMetadata(metadata));
      if (signatureKey) {
        byMarkerSignature.set(signatureKey, { group, metadata });
      }
    }
  }
  return { byInstanceId, byMarkerSignature };
}

function attachGroup(instance: CubeInstance, groupsIndex: GroupIndex): MatchedGroup | null {
  const direct = groupsIndex.byInstanceId.get(instance.instanceId) || null;
  if (direct) {
    return direct;
  }
  const markerSignature = buildMarkerSignature(instance.markerIds);
  if (!markerSignature) {
    return null;
  }
  return groupsIndex.byMarkerSignature.get(markerSignature) || null;
}

function buildIndex({
  graph,
  instanceBuilder,
  groups,
}: BuildIndexOptions): CubeInstanceIndexResult {
  const resolvedGroups = Array.isArray(groups) ? groups : getGraphGroups(graph);
  const builder = instanceBuilder || new InstanceBuilder({ logger: null });
  const groupIndex = buildGroupIndex(resolvedGroups);
  const instances = builder.build(graph);
  const instanceById = new Map<string, IndexedCubeInstance>();
  const instanceByCubeId = new Map<string, IndexedCubeInstance>();
  const instanceByMarkerId = new Map<string, IndexedCubeInstance>();
  const instanceByNodeId = new Map<string, IndexedCubeInstance>();

  const enriched = instances.map((instance) => {
    const matched = attachGroup(instance, groupIndex);
    const group = matched?.group ?? null;
    const metadata = matched?.metadata ?? null;
    const bounds = resolveInstanceBounds({
      group,
      metadata,
      nodes: instance.nodes,
      markers: instance.markers,
    });
    const entry: IndexedCubeInstance = { ...instance, group, metadata, bounds };
    if (entry.instanceId) {
      instanceById.set(entry.instanceId, entry);
    }
    if (entry.cubeId) {
      instanceByCubeId.set(entry.cubeId, entry);
    }
    for (const node of entry.nodes || []) {
      const nodeId = node?.id != null ? String(node.id) : '';
      if (nodeId && !instanceByNodeId.has(nodeId)) {
        instanceByNodeId.set(nodeId, entry);
      }
    }
    for (const markerId of entry.markerIds || []) {
      const markerKey = String(markerId);
      if (!instanceByMarkerId.has(markerKey)) {
        instanceByMarkerId.set(markerKey, entry);
      }
    }
    return entry;
  });

  return {
    instances: enriched,
    instanceById,
    instanceByCubeId,
    instanceByMarkerId,
    instanceByNodeId,
  };
}

/**
 * Coordinate cube instance index behavior for the SugarCubes UI.
 */
export class CubeInstanceIndex {
  readonly graph: ComfyGraph | null;
  readonly instanceBuilder: InstanceBuilderTarget | null;
  readonly groups: ComfyGroup[] | null;
  readonly instances: IndexedCubeInstance[];
  readonly instanceById: Map<string, IndexedCubeInstance>;
  readonly instanceByCubeId: Map<string, IndexedCubeInstance>;
  readonly instanceByMarkerId: Map<string, IndexedCubeInstance>;
  readonly instanceByNodeId: Map<string, IndexedCubeInstance>;

  constructor({
    graph,
    instanceBuilder,
    groups,
  }: {
    graph?: ComfyGraph | null;
    instanceBuilder?: InstanceBuilderTarget | null;
    groups?: ComfyGroup[] | null;
  } = {}) {
    this.graph = graph || null;
    this.instanceBuilder = instanceBuilder || null;
    this.groups = Array.isArray(groups) ? groups : null;
    const { instances, instanceById, instanceByCubeId, instanceByMarkerId, instanceByNodeId } =
      buildIndex({
        graph: this.graph,
        instanceBuilder: this.instanceBuilder,
        groups: this.groups,
      });
    this.instances = instances;
    this.instanceById = instanceById;
    this.instanceByCubeId = instanceByCubeId;
    this.instanceByMarkerId = instanceByMarkerId;
    this.instanceByNodeId = instanceByNodeId;
  }
}
