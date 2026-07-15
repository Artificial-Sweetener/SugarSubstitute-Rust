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
 * Own the SugarCubes graph integration layer in `web/comfyui/ui/graph/InstanceBuilder.js`.
 */

import { buildLinkIndex, getGraphGroups, getGraphNodes } from './GraphQuery.js';
import {
  CUBE_MARKER_KINDS,
  isCubeMarkerType,
  readCubeMarkerId,
  readCubeMarkerInstanceId,
  readCubeMarkerInstanceAlias,
  readCubeMarkerDefaultAlias,
  readCubeMarkerRevisionRef,
  readCubeMarkerVersion,
} from './CubeMarkers.js';
import { buildCubeDefinitionKey, normalizeCubeVersion } from '../core/CubeDefinitionKey.js';
import { getGroupSugarcubes } from './GroupMetadata.js';
import type { CubeGroupMetadataRecord, CubeMarkerLookup } from './GroupMetadata.js';
import type { ComfyGraph, ComfyLink, ComfyNode } from '../types/graph.js';

type CubeMarkerKind = 'input' | 'output';

interface CubeMarkerEntry {
  id: string;
  kind: CubeMarkerKind;
  node: ComfyNode;
  cubeId: string;
  cubeVersion: string;
  cubeRevisionRef: string;
  cubeDefinitionKey: string;
}

export interface CubeInstance {
  instanceId: string;
  defaultAlias: string;
  instanceAlias: string;
  targetModel: string;
  cubeId: string;
  cubeVersion: string;
  cubeRevisionRef: string;
  cubeDefinitionKey: string;
  icon: unknown;
  markerLookup: CubeMarkerLookup;
  nodeIds: string[];
  markerIds: string[];
  nodes: ComfyNode[];
  markers: ComfyNode[];
}

interface InstanceBuilderOptions {
  logger?: Pick<Console, 'warn'> | null;
  instanceIdFactory?: (() => string) | null;
}

function defaultInstanceIdFactory(): string {
  const cryptoRef = typeof globalThis !== 'undefined' ? globalThis.crypto : null;
  if (cryptoRef?.randomUUID) {
    return cryptoRef.randomUUID();
  }
  const time = Date.now().toString(36);
  const rand = Math.random().toString(36).slice(2, 10);
  return `inst_${time}_${rand}`;
}

function computeCubeSubgraph(
  cubeMarkers: readonly CubeMarkerEntry[],
  graphNodes: readonly ComfyNode[],
  outgoing: ReadonlyMap<string, ComfyLink[]>,
  incoming: ReadonlyMap<string, ComfyLink[]>,
  markerIds: ReadonlySet<string>,
): Set<string> {
  const nodeById = new Map(graphNodes.filter(Boolean).map((node) => [String(node.id), node]));
  const isMarkerId = (nodeId: unknown): boolean => markerIds.has(String(nodeId));

  const forward = new Set<string>();
  const queue: string[] = [];
  for (const marker of cubeMarkers) {
    if (marker.kind !== 'input') {
      continue;
    }
    const edges = outgoing.get(marker.id) ?? [];
    for (const edge of edges) {
      const targetId = edge.target_id ?? edge.target;
      if (targetId == null || isMarkerId(targetId)) {
        continue;
      }
      const targetKey = String(targetId);
      if (!forward.has(targetKey)) {
        forward.add(targetKey);
        queue.push(targetKey);
      }
    }
  }

  while (queue.length) {
    const current = queue.shift();
    if (current === undefined) break;
    const edges = outgoing.get(current) ?? [];
    for (const edge of edges) {
      const targetId = edge.target_id ?? edge.target;
      if (targetId == null || isMarkerId(targetId)) {
        continue;
      }
      const targetKey = String(targetId);
      if (!forward.has(targetKey)) {
        forward.add(targetKey);
        queue.push(targetKey);
      }
    }
  }

  const backward = new Set<string>();
  const backQueue: string[] = [];
  for (const marker of cubeMarkers) {
    if (marker.kind !== 'output') {
      continue;
    }
    const edges = incoming.get(marker.id) ?? [];
    for (const edge of edges) {
      const sourceId = edge.origin_id ?? edge.origin;
      if (sourceId == null || isMarkerId(sourceId)) {
        continue;
      }
      const sourceKey = String(sourceId);
      if (!backward.has(sourceKey)) {
        backward.add(sourceKey);
        backQueue.push(sourceKey);
      }
    }
  }

  while (backQueue.length) {
    const current = backQueue.shift();
    if (current === undefined) break;
    const edges = incoming.get(current) ?? [];
    for (const edge of edges) {
      const sourceId = edge.origin_id ?? edge.origin;
      if (sourceId == null || isMarkerId(sourceId)) {
        continue;
      }
      const sourceKey = String(sourceId);
      if (!backward.has(sourceKey)) {
        backward.add(sourceKey);
        backQueue.push(sourceKey);
      }
    }
  }

  const hasSources = cubeMarkers.some((marker) => marker.kind === 'input');
  const hasSinks = cubeMarkers.some((marker) => marker.kind === 'output');
  let combined = new Set<string>();
  if (hasSources && hasSinks) {
    for (const nodeId of forward) {
      if (backward.has(nodeId)) {
        combined.add(nodeId);
      }
    }
    if (!combined.size) {
      combined = new Set([...forward, ...backward]);
    }
  } else if (hasSources) {
    combined = new Set(forward);
  } else if (hasSinks) {
    combined = new Set(backward);
  }

  if (!combined.size) {
    return combined;
  }

  const expanded = new Set<string>(combined);
  const expandQueue = [...combined];
  while (expandQueue.length) {
    const current = expandQueue.shift();
    if (current === undefined) break;
    const edgesOut = outgoing.get(current) ?? [];
    for (const edge of edgesOut) {
      const targetId = edge.target_id ?? edge.target;
      if (targetId == null || isMarkerId(targetId)) {
        continue;
      }
      const targetKey = String(targetId);
      if (!expanded.has(targetKey)) {
        expanded.add(targetKey);
        expandQueue.push(targetKey);
      }
    }
    const edgesIn = incoming.get(current) ?? [];
    for (const edge of edgesIn) {
      const sourceId = edge.origin_id ?? edge.origin;
      if (sourceId == null || isMarkerId(sourceId)) {
        continue;
      }
      const sourceKey = String(sourceId);
      if (!expanded.has(sourceKey)) {
        expanded.add(sourceKey);
        expandQueue.push(sourceKey);
      }
    }
  }

  for (const nodeId of expanded) {
    const node = nodeById.get(nodeId);
    if (node && isCubeMarkerType(node)) {
      expanded.delete(nodeId);
    }
  }

  return expanded;
}

function buildMarkerMetadataLookup(
  graph: ComfyGraph | null | undefined,
): Map<string, CubeGroupMetadataRecord> {
  const lookup = new Map<string, CubeGroupMetadataRecord>();
  for (const group of getGraphGroups(graph)) {
    const metadata = getGroupSugarcubes(group);
    if (!metadata?.managed || !metadata.markers) {
      continue;
    }
    const markerIds = Object.values(metadata.markers).flatMap((value) =>
      Array.isArray(value) ? value : [],
    );
    for (const markerId of markerIds) {
      lookup.set(String(markerId), metadata);
    }
  }
  return lookup;
}

/**
 * Coordinate instance builder behavior for the SugarCubes UI.
 */
export class InstanceBuilder {
  private readonly logger: Pick<Console, 'warn'> | null;
  private readonly instanceIdFactory: () => string;

  constructor({ logger, instanceIdFactory }: InstanceBuilderOptions = {}) {
    this.logger = logger || null;
    this.instanceIdFactory =
      typeof instanceIdFactory === 'function' ? instanceIdFactory : defaultInstanceIdFactory;
  }

  build(graph: ComfyGraph | null | undefined): CubeInstance[] {
    const nodes = getGraphNodes(graph);
    const nodeById = new Map(nodes.filter(Boolean).map((node) => [String(node.id), node]));
    const { outgoing, incoming, links } = buildLinkIndex(graph);
    const markerMetadataById = buildMarkerMetadataLookup(graph);

    const cubeMap = new Map<string, CubeMarkerEntry[]>();
    const markerIds = new Set<string>();
    for (const node of nodes) {
      if (!isCubeMarkerType(node)) {
        continue;
      }
      const defaultAlias = readCubeMarkerDefaultAlias(node);
      const cubeId = readCubeMarkerId(node);
      const markerInstanceId = readCubeMarkerInstanceId(node);
      const markerInstanceAlias = readCubeMarkerInstanceAlias(node);
      const metadata = node?.id != null ? markerMetadataById.get(String(node.id)) : null;
      const metadataInstanceId =
        metadata?.managed && typeof metadata.instance_id === 'string'
          ? metadata.instance_id.trim()
          : '';
      const effectiveInstanceId = metadataInstanceId || markerInstanceId;
      const cubeVersion =
        normalizeCubeVersion(readCubeMarkerVersion(node)) ||
        normalizeCubeVersion(metadata?.cube_version);
      const metadataRevisionRef =
        typeof metadata?.cube_revision_ref === 'string' ? metadata.cube_revision_ref : '';
      const cubeRevisionRef = readCubeMarkerRevisionRef(node) || metadataRevisionRef;
      const metadataDefinitionKey =
        typeof metadata?.cube_definition_key === 'string' ? metadata.cube_definition_key : '';
      const cubeDefinitionKey =
        metadataDefinitionKey || buildCubeDefinitionKey(cubeId, cubeVersion);
      if (!defaultAlias || !cubeId) {
        continue;
      }
      const versionKey = cubeVersion ? `:version:${cubeVersion}` : '';
      let cubeKey = `cube:${cubeId}${versionKey}`;
      const instanceAliasKey = markerInstanceAlias.toLowerCase();
      if (effectiveInstanceId) {
        cubeKey = `cube:${cubeId}${versionKey}:instance:${effectiveInstanceId}`;
      } else if (instanceAliasKey) {
        cubeKey = `cube:${cubeId}${versionKey}:instance-alias:${instanceAliasKey}`;
      }
      const kind = node.type ? CUBE_MARKER_KINDS[node.type] : undefined;
      if (!kind) {
        continue;
      }
      const entry: CubeMarkerEntry = {
        id: String(node.id),
        kind,
        node,
        cubeId,
        cubeVersion,
        cubeRevisionRef,
        cubeDefinitionKey,
      };
      const list = cubeMap.get(cubeKey) ?? [];
      list.push(entry);
      cubeMap.set(cubeKey, list);
      markerIds.add(String(node.id));
    }

    const instances: CubeInstance[] = [];
    for (const cubeMarkers of cubeMap.values()) {
      const subgraphNodes = computeCubeSubgraph(cubeMarkers, nodes, outgoing, incoming, markerIds);
      const componentNodes = new Set([...subgraphNodes, ...cubeMarkers.map((marker) => marker.id)]);
      if (!componentNodes.size) {
        continue;
      }

      const adjacency = new Map<string, Set<string>>();
      for (const link of links) {
        const originId = link.origin_id ?? link.origin;
        const targetId = link.target_id ?? link.target;
        if (originId == null || targetId == null) {
          continue;
        }
        const originKey = String(originId);
        const targetKey = String(targetId);
        if (!componentNodes.has(originKey) || !componentNodes.has(targetKey)) {
          continue;
        }
        const a = adjacency.get(originKey) ?? new Set();
        a.add(targetKey);
        adjacency.set(originKey, a);
        const b = adjacency.get(targetKey) ?? new Set();
        b.add(originKey);
        adjacency.set(targetKey, b);
      }

      const seen = new Set<string>();
      for (const startId of componentNodes) {
        if (seen.has(startId)) {
          continue;
        }
        const queue: string[] = [startId];
        const component = new Set<string>();
        seen.add(startId);
        while (queue.length) {
          const current = queue.shift();
          if (current === undefined) break;
          component.add(current);
          const neighbors = adjacency.get(current);
          if (!neighbors) {
            continue;
          }
          for (const next of neighbors) {
            if (seen.has(next)) {
              continue;
            }
            seen.add(next);
            queue.push(next);
          }
        }

        const componentMarkers = cubeMarkers.filter((marker) => component.has(marker.id));
        const componentNodeIds = Array.from(component).filter((id) => subgraphNodes.has(id));
        if (!componentMarkers.length) {
          continue;
        }
        const defaultAlias = componentMarkers[0]?.node
          ? readCubeMarkerDefaultAlias(componentMarkers[0].node)
          : '';
        const instanceAlias = componentMarkers[0]?.node
          ? readCubeMarkerInstanceAlias(componentMarkers[0].node)
          : '';
        const cubeId = componentMarkers.find((marker) => marker.cubeId)?.cubeId || '';
        const definitionMetadata =
          componentMarkers
            .map((marker) =>
              marker.node?.id != null ? markerMetadataById.get(String(marker.node.id)) : null,
            )
            .find((metadata) => metadata?.managed) || null;
        const cubeVersion =
          componentMarkers.find((marker) => marker.cubeVersion)?.cubeVersion || '';
        const cubeRevisionRef =
          componentMarkers.find((marker) => marker.cubeRevisionRef)?.cubeRevisionRef || '';
        const cubeDefinitionKey =
          componentMarkers.find((marker) => marker.cubeDefinitionKey)?.cubeDefinitionKey ||
          buildCubeDefinitionKey(cubeId, cubeVersion);
        const icon = componentMarkers.find(
          (marker) =>
            marker.node?.id != null && markerMetadataById.get(String(marker.node.id))?.icon,
        );
        const definitionIcon =
          icon?.node?.id != null
            ? markerMetadataById.get(String(icon.node.id))?.icon || null
            : null;

        const markerInstanceIds = componentMarkers
          .map((marker) => readCubeMarkerInstanceId(marker.node))
          .filter(Boolean);
        const uniqueInstanceIds = Array.from(new Set(markerInstanceIds));
        let instanceId = '';
        if (uniqueInstanceIds.length === 1) {
          instanceId = uniqueInstanceIds[0] ?? '';
        } else if (!uniqueInstanceIds.length) {
          instanceId = this.instanceIdFactory();
        } else {
          uniqueInstanceIds.sort();
          instanceId = uniqueInstanceIds[0] ?? '';
          this.logger?.warn?.('SugarCubes: marker instance_id mismatch detected', {
            cubeId,
            defaultAlias,
            instanceId,
            conflicts: uniqueInstanceIds,
          });
        }
        const markerLookup = {
          inputs: componentMarkers
            .filter((marker) => marker.kind === 'input')
            .map((marker) => marker.id),
          outputs: componentMarkers
            .filter((marker) => marker.kind === 'output')
            .map((marker) => marker.id),
        };

        instances.push({
          instanceId,
          defaultAlias,
          instanceAlias,
          targetModel:
            typeof definitionMetadata?.target_model === 'string'
              ? definitionMetadata.target_model.trim()
              : '',
          cubeId,
          cubeVersion,
          cubeRevisionRef,
          cubeDefinitionKey,
          icon: definitionIcon,
          markerLookup,
          nodeIds: componentNodeIds,
          markerIds: componentMarkers.map((marker) => marker.id),
          nodes: componentNodeIds
            .map((id) => nodeById.get(id))
            .filter((node): node is ComfyNode => Boolean(node)),
          markers: componentMarkers.map((marker) => marker.node),
        });
      }
    }

    return instances;
  }
}
