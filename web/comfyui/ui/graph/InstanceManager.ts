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
 * Own the SugarCubes graph integration layer in `web/comfyui/ui/graph/InstanceManager.js`.
 */

import { getNodeCenter, isPointInBounds, readGroupBounds } from './Bounds.js';
import {
  CUBE_INSTANCE_HEADER_HEIGHT,
  CUBE_INSTANCE_AUTO_MIN_MARGINS,
  CUBE_INSTANCE_PADDING,
  CUBE_INSTANCE_TOP_EXTRA,
  computeInstanceBounds,
  computeVisualContentBounds,
  contentFitsWithinBounds,
  expandBoundsForContentMargins,
  inflateInstanceBounds,
  resolveChromeBoundsFromContent,
  resolveNewInstanceBounds,
} from './CubeBounds.js';
import { buildMarkerSignature, readMarkerIdsFromMetadata } from '../layout/CubeInstanceIndex.js';
import { getGraphGroups } from './GraphQuery.js';
import { InstanceBuilder } from './InstanceBuilder.js';
import { allocateGraphInstanceAliases } from './AliasAllocator.js';
import {
  allocateUniqueInstanceAlias,
  ensureGroupTitleWatcher,
  syncInstanceAlias,
} from './InstanceAliasSync.js';
import {
  ensureGroupSerialization,
  flattenCubeGroupMetadata,
  getGroupSugarcubes,
  setGroupSugarcubes,
  resolveInstanceDisplayName,
  writeCubeDefinitionMetadata,
  writeCubeInstanceMetadata,
} from './GroupMetadata.js';
import { updateMarkersForIds } from './CubeMarkers.js';
import { buildCubeDefinitionKey, normalizeRevisionRef } from '../core/CubeDefinitionKey.js';
import { isRecord } from '../types/common.js';
import type { CubeBoundsHeader, CubeBoundsPadding } from './CubeBounds.js';
import type { CubeInstance } from './InstanceBuilder.js';
import type { CubeGroupMetadataRecord } from './GroupMetadata.js';
import type { RectBounds } from '../types/common.js';
import type { ComfyGraph, ComfyGroup } from '../types/graph.js';

interface ManagedGroup extends ComfyGroup {
  __sugarcubes_imported?: boolean;
}

export interface InstanceAdapter {
  getConsole?(): Pick<Console, 'warn'> | null;
  getLiteGraph?(): { LGraphGroup?: new (title?: string) => ManagedGroup } | null;
}

interface InstanceEvents {
  emit?(event: string, payload: unknown): unknown;
}

interface InstanceScheduler {
  raf?(callback: FrameRequestCallback): number | null;
}

interface InstanceBuilderTarget {
  build(graph: ComfyGraph | null | undefined): CubeInstance[];
}

interface InstanceManagerOptions {
  adapter: InstanceAdapter;
  events?: InstanceEvents | null;
  scheduler?: InstanceScheduler | null;
  instanceBuilder?: InstanceBuilderTarget | null;
  requestDirtyRefresh?: ((options: InstanceRefreshOptions) => unknown) | null;
}

interface InstanceRefreshOptions {
  graph?: ComfyGraph | null | undefined;
  reason?: string | undefined;
  force?: boolean;
}

interface InstanceMatch {
  instance: CubeInstance;
  group: ManagedGroup | null;
  order: number;
}

const CUBE_INSTANCE_SCHEMA = 5;
const CUBE_INSTANCE_GROUP_COLOR = '#3f789e';
const CUBE_INSTANCE_GROUP_BG = '#3f5159';
function readNumber(value: unknown, fallback: number): number;
function readNumber(value: unknown, fallback: null): number | null;
function readNumber(value: unknown, fallback: number | null = null): number | null {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function normalizePadding(padding: unknown): CubeBoundsPadding {
  const source = isRecord(padding) ? padding : {};
  return {
    x: readNumber(source.x, CUBE_INSTANCE_PADDING.x),
    y: readNumber(source.y, CUBE_INSTANCE_PADDING.y),
    top_extra: readNumber(source.top_extra, CUBE_INSTANCE_TOP_EXTRA),
  };
}

function normalizeHeader(header: unknown): CubeBoundsHeader {
  const source = isRecord(header) ? header : {};
  return {
    height: readNumber(source.height, CUBE_INSTANCE_HEADER_HEIGHT),
  };
}

function normalizeBoundsGeometry(bounds: unknown): RectBounds | null {
  const source = isRecord(bounds) ? bounds : {};
  const x = readNumber(source.x, null);
  const y = readNumber(source.y, null);
  const w = readNumber(source.w, null);
  const h = readNumber(source.h, null);
  if (x === null || y === null || w === null || h === null) {
    return null;
  }
  return { x, y, w, h };
}

function readGroupBoundsGeometry(group: ComfyGroup | null | undefined): RectBounds | null {
  const bounds = readGroupBounds(group);
  if (!bounds || bounds.length < 4) {
    return null;
  }
  const x = readNumber(bounds[0], null);
  const y = readNumber(bounds[1], null);
  const w = readNumber(bounds[2], null);
  const h = readNumber(bounds[3], null);
  if (x === null || y === null || w === null || h === null) {
    return null;
  }
  return { x, y, w, h };
}

function findReusableGroup(
  instance: CubeInstance,
  groups: readonly ManagedGroup[],
): ManagedGroup | null {
  let best: ManagedGroup | null = null;
  let bestArea: number | null = null;
  for (const group of groups) {
    if (!group || getGroupSugarcubes(group)) {
      continue;
    }
    const bounds = readGroupBounds(group);
    if (!bounds) {
      continue;
    }
    const allInside = [...instance.nodes, ...instance.markers].every((node) => {
      const center = getNodeCenter(node);
      return center ? isPointInBounds(center, bounds) : false;
    });
    if (!allInside) {
      continue;
    }
    const area = bounds[2] * bounds[3];
    if (bestArea == null || area < bestArea) {
      best = group;
      bestArea = area;
    }
  }
  return best;
}

/**
 * Merge two instance ID lists without changing their display values.
 */
function mergeLookupLists<T>(left: readonly T[] = [], right: readonly T[] = []): T[] {
  return Array.from(
    new Set([...(Array.isArray(left) ? left : []), ...(Array.isArray(right) ? right : [])]),
  );
}

/**
 * Preserve legacy repair for marker-only reusable groups with mismatched marker IDs.
 */
function mergeInstancesForReusableMarkerGroups(
  instanceMatches: readonly InstanceMatch[],
): InstanceMatch[] {
  const grouped = new Map<ManagedGroup, InstanceMatch[]>();
  const passthrough: InstanceMatch[] = [];
  for (const match of instanceMatches) {
    const metadata = getGroupSugarcubes(match.group);
    if (
      !match.group ||
      metadata ||
      !match.instance ||
      (Array.isArray(match.instance.nodeIds) && match.instance.nodeIds.length > 0)
    ) {
      passthrough.push(match);
      continue;
    }
    const list = grouped.get(match.group) ?? [];
    list.push(match);
    grouped.set(match.group, list);
  }

  const merged: InstanceMatch[] = [...passthrough];
  for (const matches of grouped.values()) {
    if (matches.length === 1) {
      const onlyMatch = matches[0];
      if (onlyMatch) {
        merged.push(onlyMatch);
      }
      continue;
    }
    const definitionKeys = new Set(matches.map((match) => match.instance.cubeDefinitionKey || ''));
    if (definitionKeys.size > 1) {
      merged.push(...matches);
      continue;
    }
    const ordered = [...matches].sort((left, right) => left.order - right.order);
    const firstMatch = ordered[0];
    if (!firstMatch) {
      continue;
    }
    const canonical = { ...firstMatch.instance };
    canonical.instanceId =
      matches
        .map((match) => match.instance.instanceId)
        .filter(Boolean)
        .sort()[0] ?? canonical.instanceId;
    canonical.markerLookup = {
      inputs: [],
      outputs: [],
    };
    canonical.nodeIds = [];
    canonical.markerIds = [];
    canonical.nodes = [];
    canonical.markers = [];
    for (const match of ordered) {
      canonical.markerLookup.inputs = mergeLookupLists(
        canonical.markerLookup.inputs,
        match.instance.markerLookup?.inputs,
      );
      canonical.markerLookup.outputs = mergeLookupLists(
        canonical.markerLookup.outputs,
        match.instance.markerLookup?.outputs,
      );
      canonical.markerIds = mergeLookupLists(canonical.markerIds, match.instance.markerIds);
      canonical.markers = mergeLookupLists(canonical.markers, match.instance.markers);
    }
    merged.push({ ...firstMatch, instance: canonical });
  }
  return merged.sort((left, right) => left.order - right.order);
}

function applyInstanceGroup(
  instance: CubeInstance,
  group: ManagedGroup | null,
  graph: ComfyGraph,
  adapter: InstanceAdapter,
  events: InstanceEvents | null,
  requestDirtyRefresh: ((options: InstanceRefreshOptions) => unknown) | null,
  resolvedInstanceAlias: string | undefined,
): ManagedGroup | null {
  const contentBounds = computeInstanceBounds(instance.nodes, instance.markers);
  const visualContentBounds = computeVisualContentBounds(instance.nodes, instance.markers);
  const existing = group ? getGroupSugarcubes(group) : null;
  const cleanedExisting = existing ? { ...existing } : null;
  const existingInstanceId =
    typeof cleanedExisting?.instance_id === 'string' ? cleanedExisting.instance_id.trim() : '';
  const canonicalInstanceId = existingInstanceId || instance.instanceId;
  const previousDefaultAlias =
    typeof cleanedExisting?.default_alias === 'string' ? cleanedExisting.default_alias.trim() : '';
  const existingInstanceAlias =
    typeof cleanedExisting?.instance_alias === 'string'
      ? cleanedExisting.instance_alias.trim()
      : '';
  const canonicalDefaultAlias =
    instance.defaultAlias || readMetadataString(cleanedExisting, 'default_alias');
  const canonicalTargetModel =
    instance?.targetModel ||
    (typeof cleanedExisting?.target_model === 'string' ? cleanedExisting.target_model : '');
  const cubeVersion =
    instance?.cubeVersion ||
    (typeof cleanedExisting?.cube_version === 'string' ? cleanedExisting.cube_version : '');
  const cubeRevisionRef = normalizeRevisionRef(
    instance?.cubeRevisionRef || cleanedExisting?.cube_revision_ref,
  );
  const cubeDefinitionKey =
    instance?.cubeDefinitionKey ||
    readMetadataString(cleanedExisting, 'cube_definition_key') ||
    buildCubeDefinitionKey(instance.cubeId, cubeVersion);
  const definitionIcon =
    instance?.icon && typeof instance.icon === 'object'
      ? instance.icon
      : cleanedExisting?.icon && typeof cleanedExisting.icon === 'object'
        ? cleanedExisting.icon
        : null;
  const canonicalInstanceAlias =
    (typeof resolvedInstanceAlias === 'string' && resolvedInstanceAlias.trim()) ||
    existingInstanceAlias ||
    instance?.instanceAlias ||
    canonicalDefaultAlias ||
    instance?.cubeId ||
    'SugarCube';
  const existingBounds = normalizeBoundsGeometry(cleanedExisting?.bounds);
  const existingGroupBounds = readGroupBoundsGeometry(group);
  const existingBoundsRecord = isRecord(cleanedExisting?.bounds) ? cleanedExisting.bounds : {};
  const resolvedPadding = normalizePadding(existingBoundsRecord.padding);
  const resolvedHeader = normalizeHeader(existingBoundsRecord.header);
  const contentDerivedBounds = resolveChromeBoundsFromContent({
    nodes: instance.nodes,
    markers: instance.markers,
    padding: resolvedPadding,
    header: resolvedHeader,
  });
  let canonicalBounds = existingGroupBounds || existingBounds;
  let usedNewBoundsResolver = false;
  if (contentDerivedBounds) {
    canonicalBounds = contentDerivedBounds;
    usedNewBoundsResolver = true;
  } else if (!canonicalBounds) {
    if (cleanedExisting) {
      if (!contentBounds) {
        return null;
      }
      canonicalBounds = inflateInstanceBounds(contentBounds, {
        ...resolvedPadding,
        header: { ...resolvedHeader },
      });
    } else {
      canonicalBounds = resolveNewInstanceBounds({
        nodes: instance.nodes,
        markers: instance.markers,
        padding: resolvedPadding,
        header: resolvedHeader,
      });
      usedNewBoundsResolver = true;
    }
    if (!canonicalBounds) {
      return null;
    }
  }
  const containmentBounds = contentBounds || visualContentBounds;
  if (
    !usedNewBoundsResolver &&
    !contentFitsWithinBounds(canonicalBounds, containmentBounds) &&
    containmentBounds
  ) {
    canonicalBounds = inflateInstanceBounds(containmentBounds, {
      ...resolvedPadding,
      header: { ...resolvedHeader },
    });
  }
  if (!canonicalBounds) {
    return null;
  }
  if (!usedNewBoundsResolver && !cleanedExisting && (visualContentBounds || contentBounds)) {
    const minimumMargins = {
      left: CUBE_INSTANCE_AUTO_MIN_MARGINS.left,
      right: CUBE_INSTANCE_AUTO_MIN_MARGINS.right,
      bottom: CUBE_INSTANCE_AUTO_MIN_MARGINS.bottom,
      top:
        resolvedPadding.y +
        resolvedPadding.top_extra +
        resolvedHeader.height +
        CUBE_INSTANCE_AUTO_MIN_MARGINS.innerTop,
    };
    canonicalBounds =
      expandBoundsForContentMargins(
        canonicalBounds,
        visualContentBounds || contentBounds,
        minimumMargins,
      ) ?? canonicalBounds;
  }
  const metadataSeed = {
    ...(cleanedExisting || {}),
    schema: CUBE_INSTANCE_SCHEMA,
    managed: true,
  };
  const definitionMetadata = writeCubeDefinitionMetadata(metadataSeed, {
    cube_id: instance.cubeId,
    default_alias: canonicalDefaultAlias,
    target_model: canonicalTargetModel,
    cube_version: cubeVersion,
    cube_revision_ref: cubeRevisionRef,
    cube_definition_key: cubeDefinitionKey,
    ...(isRecord(definitionIcon) ? { icon: definitionIcon } : {}),
  });
  const ownedMetadata = writeCubeInstanceMetadata(definitionMetadata, {
    instance_id: canonicalInstanceId,
    instance_alias: canonicalInstanceAlias,
    markers: instance.markerLookup,
    nodes: instance.nodeIds,
    bounds: {
      x: canonicalBounds.x,
      y: canonicalBounds.y,
      w: canonicalBounds.w,
      h: canonicalBounds.h,
      padding: { ...resolvedPadding },
      header: { ...resolvedHeader },
    },
  });
  const metadata = flattenCubeGroupMetadata(ownedMetadata, cleanedExisting);

  let targetGroup = group;
  if (!targetGroup) {
    const liteGraph = adapter?.getLiteGraph?.() || null;
    if (!liteGraph?.LGraphGroup) {
      return null;
    }
    const displayName = resolveInstanceDisplayName({
      metadata,
      fallback: 'SugarCube',
    });
    targetGroup = new liteGraph.LGraphGroup(displayName || 'SugarCube');
    graph.add?.(targetGroup);
    if (!targetGroup.color) {
      targetGroup.color = CUBE_INSTANCE_GROUP_COLOR;
    }
    if (!targetGroup.bgcolor) {
      targetGroup.bgcolor = CUBE_INSTANCE_GROUP_BG;
    }
  }

  targetGroup.pos = [canonicalBounds.x, canonicalBounds.y];
  targetGroup.size = [canonicalBounds.w, canonicalBounds.h];
  setGroupSugarcubes(targetGroup, metadata);
  if (targetGroup.__sugarcubes_imported) {
    delete targetGroup.__sugarcubes_imported;
  }
  const metadataInstanceId = readMetadataString(metadata, 'instance_id');
  const metadataInstanceAlias = readMetadataString(metadata, 'instance_alias');
  const metadataCubeId = readMetadataString(metadata, 'cube_id');
  if (metadataInstanceId) {
    updateMarkersForIds(graph, instance.markerIds, {
      instanceId: metadataInstanceId,
      instanceAlias: metadataInstanceAlias,
      cubeVersion: metadata.cube_version,
      cubeRevisionRef: metadata.cube_revision_ref,
    });
  }
  ensureGroupTitleWatcher(targetGroup, (groupRef, next) => {
    const sugarcubes = getGroupSugarcubes(groupRef);
    const cubeId = typeof sugarcubes?.cube_id === 'string' ? sugarcubes.cube_id.trim() : '';
    if (!cubeId) {
      return;
    }
    const resolved = allocateUniqueInstanceAlias(graph, next, {
      currentInstanceId: readMetadataString(sugarcubes, 'instance_id'),
      currentGroup: groupRef,
    });
    syncInstanceAlias({
      graph,
      group: groupRef,
      metadata: sugarcubes,
      cubeId,
      instanceAlias: resolved,
      events,
      requestDirtyRefresh,
    });
  });
  syncInstanceAlias({
    graph,
    group: targetGroup,
    metadata,
    cubeId: metadataCubeId,
    instanceAlias: metadataInstanceAlias,
    // The manager is already reconciling this instance; emitting a refresh request here
    // would schedule a redundant second pass. The title watcher above still publishes
    // refresh requests for user-initiated alias changes.
    requestDirtyRefresh,
  });
  const metadataDefaultAlias = readMetadataString(metadata, 'default_alias');
  if (metadataCubeId && previousDefaultAlias && previousDefaultAlias !== metadataDefaultAlias) {
    events?.emit?.('cube:default-alias:changed', {
      cubeId: metadataCubeId,
      defaultAlias: metadataDefaultAlias,
    });
  }
  instance.instanceId = metadataInstanceId;
  return targetGroup;
}

function buildCubeInstanceSignature(instances: readonly CubeInstance[]): string {
  const parts = instances
    .map((instance) => {
      const ids = instance.nodeIds.concat(instance.markerIds).sort();
      return `${instance.instanceId}:${instance.cubeDefinitionKey || instance.cubeId}:${ids.join(',')}`;
    })
    .sort();
  return parts.join('|');
}

function buildGroupTitleSignature(groups: readonly ComfyGroup[]): string {
  return groups
    .map((group) => {
      const data = getGroupSugarcubes(group);
      if (!data?.managed || !data.instance_id) {
        return '';
      }
      const title = typeof group?.title === 'string' ? group.title.trim() : '';
      return `${data.instance_id}:${title}`;
    })
    .filter(Boolean)
    .sort()
    .join('|');
}

/**
 * Coordinate instance manager behavior for the SugarCubes UI.
 */
export class InstanceManager {
  private readonly adapter: InstanceAdapter;
  private readonly events: InstanceEvents | null;
  private readonly scheduler: InstanceScheduler | null;
  readonly instanceBuilder: InstanceBuilderTarget;
  private readonly requestDirtyRefresh: ((options: InstanceRefreshOptions) => unknown) | null;
  private scheduled: boolean;
  private pendingForce: boolean;
  private lastSignature: string | null;

  constructor({
    adapter,
    events = null,
    scheduler = null,
    instanceBuilder = null,
    requestDirtyRefresh = null,
  }: InstanceManagerOptions) {
    this.adapter = adapter;
    this.events = events;
    this.scheduler = scheduler;
    this.instanceBuilder =
      instanceBuilder || new InstanceBuilder({ logger: adapter.getConsole?.() ?? null });
    this.requestDirtyRefresh =
      typeof requestDirtyRefresh === 'function' ? requestDirtyRefresh : null;
    this.scheduled = false;
    this.pendingForce = false;
    this.lastSignature = null;
  }

  setup(): void {
    ensureGroupSerialization(this.adapter);
  }

  scheduleRefresh({ graph, reason, force = false }: InstanceRefreshOptions = {}): void {
    if (force) {
      this.pendingForce = true;
    }
    if (this.scheduled) {
      return;
    }
    this.scheduled = true;
    this.scheduler?.raf?.(() => {
      const shouldForce = this.pendingForce;
      this.scheduled = false;
      this.pendingForce = false;
      this.refresh({ graph, reason, force: shouldForce });
    });
  }

  refresh({ graph, force = false }: InstanceRefreshOptions = {}): void {
    const targetGraph = graph;
    if (!targetGraph) {
      return;
    }
    const liteGraph = this.adapter?.getLiteGraph?.() || null;
    if (!liteGraph?.LGraphGroup) {
      return;
    }
    const groups = getGraphGroups(targetGraph);
    const instances = this.instanceBuilder.build(targetGraph);
    const signature = `${buildCubeInstanceSignature(instances)}::${buildGroupTitleSignature(
      groups,
    )}`;
    if (!force && this.lastSignature === signature) {
      return;
    }
    this.lastSignature = signature;
    const existing = new Map<string, ManagedGroup>();
    const existingByMarkers = new Map<string, ManagedGroup>();
    for (const group of groups) {
      const data = getGroupSugarcubes(group);
      const instanceId = readMetadataString(data, 'instance_id');
      if (instanceId) {
        existing.set(instanceId, group);
      }
      if (data?.managed) {
        const signatureKey = buildMarkerSignature(readMarkerIdsFromMetadata(data));
        if (signatureKey) {
          existingByMarkers.set(signatureKey, group);
        }
      }
    }

    const instanceMatches: InstanceMatch[] = [];
    const claimedGroups = new Set<ManagedGroup>();
    for (const [index, instance] of instances.entries()) {
      const markerSignature = buildMarkerSignature(instance.markerIds);
      let match =
        existing.get(instance.instanceId) ||
        (markerSignature ? existingByMarkers.get(markerSignature) : null) ||
        null;
      if (match && getGroupSugarcubes(match) && claimedGroups.has(match)) {
        match = null;
      }
      if (!match) {
        const availableGroups = groups.filter((group) => !claimedGroups.has(group));
        match = findReusableGroup(instance, availableGroups);
      }
      if (match && getGroupSugarcubes(match)) {
        claimedGroups.add(match);
      }
      instanceMatches.push({ instance, group: match, order: index });
    }

    const resolvedMatches = mergeInstancesForReusableMarkerGroups(instanceMatches);
    const instanceAliases = allocateGraphInstanceAliases(resolvedMatches);

    const appliedGroups = new Set<ManagedGroup>();
    for (const { instance, group: match } of resolvedMatches) {
      const applied = applyInstanceGroup(
        instance,
        match,
        targetGraph,
        this.adapter,
        this.events,
        this.requestDirtyRefresh,
        instanceAliases.get(instance.instanceId),
      );
      if (applied) {
        appliedGroups.add(applied);
      }
    }

    for (const group of groups) {
      const data = getGroupSugarcubes(group);
      if (!data?.managed) {
        continue;
      }
      if (!appliedGroups.has(group)) {
        targetGraph.remove?.(group);
      }
    }

    this.events?.emit?.('cube:instances:updated', {
      graph: targetGraph,
      instances: resolvedMatches.map((match) => match.instance),
    });
  }
}

function readMetadataString(
  metadata: CubeGroupMetadataRecord | null | undefined,
  key: string,
): string {
  const value = metadata?.[key];
  return typeof value === 'string' ? value.trim() : '';
}
