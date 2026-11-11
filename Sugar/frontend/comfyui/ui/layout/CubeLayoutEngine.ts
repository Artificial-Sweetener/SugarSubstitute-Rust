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
 * Own the SugarCubes layout orchestration layer in `frontend/comfyui/ui/layout/CubeLayoutEngine.js`.
 */

import { CUBE_MARKER_KINDS } from '../graph/CubeMarkers.js';
import { buildLinkIndex } from '../graph/GraphQuery.js';
import { isRecord } from '../types/common.js';
import type { RectBounds, Vec2 } from '../types/common.js';
import type { ComfyGraph, ComfyNode, GraphId } from '../types/graph.js';
import type { MoveDelta } from './CubeMover.js';

export interface LayoutEntry {
  instanceId: string;
  bounds?: unknown;
  markers?: ComfyNode[];
}

export interface LayoutIndex {
  instances?: LayoutEntry[];
  instanceByMarkerId?: ReadonlyMap<string, LayoutEntry>;
  graph?: ComfyGraph | null;
}

export interface ProximityMatch {
  outputId?: GraphId | null | undefined;
  inputId?: GraphId | null | undefined;
}

export interface ChainOrderStrategy {
  graph?: ComfyGraph | null;
  proximityMatches?: readonly ProximityMatch[] | null;
  anchorInstanceId?: GraphId | null;
}

export interface LayoutPlacement {
  instanceId: string;
  x: number;
  y: number;
}

export interface LayoutPoint {
  x: number;
  y: number;
}

export interface SwapLayoutOptions {
  origin?: readonly number[] | null;
  minGap?: unknown;
  gap?: unknown;
  gaps?: readonly unknown[] | null;
}

interface EdgeIndex {
  outgoing: Map<string, Set<string>>;
  incoming: Map<string, Set<string>>;
}

interface EdgeIndexOptions {
  graph?: ComfyGraph | null | undefined;
  instanceByMarkerId?: ReadonlyMap<string, LayoutEntry> | undefined;
  proximityMatches?: readonly ProximityMatch[] | null | undefined;
}

function readNumber(value: unknown, fallback: number): number;
function readNumber(value: unknown, fallback: null): number | null;
function readNumber(value: unknown, fallback: number | null = 0): number | null {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function normalizeBounds(bounds: unknown): RectBounds | null {
  if (!isRecord(bounds)) {
    return null;
  }
  const x = readNumber(bounds.x, null);
  const y = readNumber(bounds.y, null);
  const w = readNumber(bounds.w, null);
  const h = readNumber(bounds.h, null);
  if (x === null || y === null || w === null || h === null) {
    return null;
  }
  return { x, y, w, h };
}

function resolveEntry(
  order: readonly LayoutEntry[],
  value: LayoutEntry | GraphId | null | undefined,
): LayoutEntry | null {
  if (!value) {
    return null;
  }
  if (typeof value === 'object' && value.instanceId) {
    return value;
  }
  const id = String(value);
  return (order || []).find((entry) => entry?.instanceId === id) || null;
}

function markerKind(node: ComfyNode | null | undefined): 'input' | 'output' | 'inherit' | '' {
  return node?.type ? CUBE_MARKER_KINDS[node.type] || '' : '';
}

function addDirectedEdge(
  outgoing: Map<string, Set<string>>,
  incoming: Map<string, Set<string>>,
  fromId: string,
  toId: string,
): void {
  if (!fromId || !toId || fromId === toId) {
    return;
  }
  const outSet = outgoing.get(fromId) ?? new Set();
  outSet.add(toId);
  outgoing.set(fromId, outSet);
  const inSet = incoming.get(toId) ?? new Set();
  inSet.add(fromId);
  incoming.set(toId, inSet);
}

function buildEdgeIndex({
  graph,
  instanceByMarkerId,
  proximityMatches,
}: EdgeIndexOptions): EdgeIndex {
  const outgoing = new Map<string, Set<string>>();
  const incoming = new Map<string, Set<string>>();
  if (!instanceByMarkerId) {
    return { outgoing, incoming };
  }
  if (graph) {
    const { links } = buildLinkIndex(graph);
    for (const link of links) {
      const originId = link.origin_id ?? link.origin;
      const targetId = link.target_id ?? link.target;
      if (originId == null || targetId == null) {
        continue;
      }
      const originInstance = instanceByMarkerId.get(String(originId));
      const targetInstance = instanceByMarkerId.get(String(targetId));
      if (!originInstance || !targetInstance) {
        continue;
      }
      const originMarker = originInstance.markers?.find(
        (marker) => String(marker?.id) === String(originId),
      );
      const targetMarker = targetInstance.markers?.find(
        (marker) => String(marker?.id) === String(targetId),
      );
      if (!originMarker || !targetMarker) {
        continue;
      }
      if (markerKind(originMarker) !== 'output') {
        continue;
      }
      const targetKind = markerKind(targetMarker);
      if (targetKind !== 'input' && targetKind !== 'inherit') {
        continue;
      }
      addDirectedEdge(outgoing, incoming, originInstance.instanceId, targetInstance.instanceId);
    }
  }
  const virtualMatches = Array.isArray(proximityMatches) ? proximityMatches : [];
  for (const match of virtualMatches) {
    const outputMarkerId = match?.outputId;
    const inputMarkerId = match?.inputId;
    if (outputMarkerId == null || inputMarkerId == null) {
      continue;
    }
    const outputInstance = instanceByMarkerId.get(String(outputMarkerId));
    const inputInstance = instanceByMarkerId.get(String(inputMarkerId));
    if (!outputInstance || !inputInstance) {
      continue;
    }
    addDirectedEdge(outgoing, incoming, outputInstance.instanceId, inputInstance.instanceId);
  }
  return { outgoing, incoming };
}

function isLinearChain(
  instances: readonly LayoutEntry[],
  outgoing: ReadonlyMap<string, ReadonlySet<string>>,
  incoming: ReadonlyMap<string, ReadonlySet<string>>,
): { ok: boolean; head: string | null } {
  const ids = instances.map((entry) => entry.instanceId).filter(Boolean);
  if (!ids.length) {
    return { ok: false, head: null };
  }
  let head: string | null = null;
  for (const id of ids) {
    const outCount = outgoing.get(id)?.size ?? 0;
    const inCount = incoming.get(id)?.size ?? 0;
    if (outCount > 1 || inCount > 1) {
      return { ok: false, head: null };
    }
    if (inCount === 0) {
      if (head && head !== id) {
        return { ok: false, head: null };
      }
      head = id;
    }
  }
  if (!head) {
    return { ok: false, head: null };
  }

  const visited = new Set<string>();
  let cursor: string | null = head;
  while (cursor) {
    if (visited.has(cursor)) {
      return { ok: false, head: null };
    }
    visited.add(cursor);
    const out = outgoing.get(cursor);
    if (!out || !out.size) {
      break;
    }
    if (out.size > 1) {
      return { ok: false, head: null };
    }
    cursor = Array.from(out)[0] ?? null;
  }
  if (visited.size !== ids.length) {
    return { ok: false, head: null };
  }
  return { ok: true, head };
}

function sortBySpatial(instances: readonly LayoutEntry[]): LayoutEntry[] {
  return [...instances].sort((a, b) => {
    const boundsA = normalizeBounds(a.bounds);
    const boundsB = normalizeBounds(b.bounds);
    const ax = boundsA ? boundsA.x : Number.POSITIVE_INFINITY;
    const bx = boundsB ? boundsB.x : Number.POSITIVE_INFINITY;
    if (ax !== bx) {
      return ax - bx;
    }
    const ay = boundsA ? boundsA.y : Number.POSITIVE_INFINITY;
    const by = boundsB ? boundsB.y : Number.POSITIVE_INFINITY;
    if (ay !== by) {
      return ay - by;
    }
    return String(a.instanceId).localeCompare(String(b.instanceId));
  });
}

function collectConnectedComponent(
  instances: readonly LayoutEntry[],
  outgoing: ReadonlyMap<string, ReadonlySet<string>>,
  incoming: ReadonlyMap<string, ReadonlySet<string>>,
  anchorInstanceId: GraphId | null | undefined,
): Set<string> | null {
  const anchorId =
    typeof anchorInstanceId === 'string' || typeof anchorInstanceId === 'number'
      ? String(anchorInstanceId).trim()
      : '';
  if (!anchorId) {
    return null;
  }
  const validIds = new Set(
    instances
      .map((entry) => (entry?.instanceId ? String(entry.instanceId).trim() : ''))
      .filter(Boolean),
  );
  if (!validIds.has(anchorId)) {
    return null;
  }
  const visited = new Set<string>();
  const queue = [anchorId];
  while (queue.length) {
    const current = queue.shift();
    if (!current || visited.has(current)) {
      continue;
    }
    visited.add(current);
    const outgoingNeighbors = outgoing.get(current);
    const incomingNeighbors = incoming.get(current);
    const neighbors = [
      ...(outgoingNeighbors ? Array.from(outgoingNeighbors) : []),
      ...(incomingNeighbors ? Array.from(incomingNeighbors) : []),
    ];
    for (const neighbor of neighbors) {
      if (!validIds.has(neighbor) || visited.has(neighbor)) {
        continue;
      }
      queue.push(neighbor);
    }
  }
  return visited;
}

/**
 * Derive chain order.
 */
export function deriveChainOrder(
  index: LayoutIndex | null | undefined,
  strategy: ChainOrderStrategy = {},
): LayoutEntry[] {
  const sourceInstances = Array.isArray(index?.instances) ? index.instances : [];
  if (!sourceInstances.length) {
    return [];
  }
  const graph = strategy?.graph || index?.graph || null;
  const { outgoing, incoming } = buildEdgeIndex({
    graph,
    instanceByMarkerId: index?.instanceByMarkerId,
    proximityMatches: strategy?.proximityMatches,
  });
  const connectedIds = collectConnectedComponent(
    sourceInstances,
    outgoing,
    incoming,
    strategy?.anchorInstanceId,
  );
  const instances = connectedIds
    ? sourceInstances.filter((entry) => connectedIds.has(entry?.instanceId))
    : sourceInstances;
  if (!instances.length) {
    return [];
  }
  const linear = isLinearChain(instances, outgoing, incoming);
  if (!linear.ok) {
    return sortBySpatial(instances);
  }
  const order: LayoutEntry[] = [];
  let cursor = linear.head;
  const byId = new Map(instances.map((entry) => [entry.instanceId, entry]));
  while (cursor) {
    const entry = byId.get(cursor);
    if (!entry) {
      break;
    }
    order.push(entry);
    const out = outgoing.get(cursor);
    if (!out || !out.size) {
      break;
    }
    cursor = Array.from(out)[0] ?? null;
  }
  if (order.length !== instances.length) {
    return sortBySpatial(instances);
  }
  return order;
}

/**
 * Lay out from order.
 */
export function layoutFromOrder(
  order: readonly LayoutEntry[] | null | undefined,
  origin: readonly unknown[] | null | undefined,
  gap: unknown,
): LayoutPlacement[] {
  const entries = Array.isArray(order) ? order : [];
  const baseX = readNumber(origin?.[0], 0);
  const baseY = readNumber(origin?.[1], 0);
  const spacing = readNumber(gap, 0);
  const placements: LayoutPlacement[] = [];
  let cursorX = baseX;
  for (const entry of entries) {
    const bounds = normalizeBounds(entry?.bounds);
    const width = bounds ? bounds.w : 0;
    if (entry?.instanceId) {
      placements.push({ instanceId: entry.instanceId, x: cursorX, y: baseY });
    }
    cursorX += width + spacing;
  }
  return placements;
}

function resolveLayoutOrigin(order: readonly LayoutEntry[]): Vec2 {
  let minX = Infinity;
  let minY = Infinity;
  for (const entry of order || []) {
    const bounds = normalizeBounds(entry?.bounds);
    if (!bounds) {
      continue;
    }
    minX = Math.min(minX, bounds.x);
    minY = Math.min(minY, bounds.y);
  }
  if (!Number.isFinite(minX) || !Number.isFinite(minY)) {
    return [0, 0];
  }
  return [minX, minY];
}

function buildGapSequence(
  order: readonly LayoutEntry[],
  minGap: unknown,
  gapValue: unknown,
  gaps: readonly unknown[] | null | undefined,
): number[] {
  const resolvedMinGap = readNumber(minGap, 0);
  if (Array.isArray(gaps) && gaps.length) {
    return gaps.map((value) => Math.max(resolvedMinGap, readNumber(value, resolvedMinGap)));
  }
  if (typeof gapValue === 'number' && Number.isFinite(gapValue)) {
    return Array(Math.max(0, (order?.length || 0) - 1)).fill(Math.max(resolvedMinGap, gapValue));
  }
  const sequence: number[] = [];
  for (let idx = 0; idx < (order?.length || 0) - 1; idx += 1) {
    const current = normalizeBounds(order[idx]?.bounds);
    const next = normalizeBounds(order[idx + 1]?.bounds);
    if (!current || !next) {
      sequence.push(resolvedMinGap);
      continue;
    }
    const spacing = next.x - (current.x + current.w);
    const resolved = Number.isFinite(spacing) ? spacing : resolvedMinGap;
    sequence.push(Math.max(resolvedMinGap, resolved));
  }
  return sequence;
}

function layoutFromOrderWithGaps(
  order: readonly LayoutEntry[],
  origin: readonly unknown[] | null,
  gaps: readonly unknown[],
  minGap: unknown,
): LayoutPlacement[] {
  const entries = Array.isArray(order) ? order : [];
  const base = Array.isArray(origin) ? origin : resolveLayoutOrigin(entries);
  const baseX = readNumber(base?.[0], 0);
  const baseY = readNumber(base?.[1], 0);
  const resolvedMinGap = readNumber(minGap, 0);
  const placements: LayoutPlacement[] = [];
  let cursorX = baseX;
  for (let idx = 0; idx < entries.length; idx += 1) {
    const entry = entries[idx];
    const bounds = normalizeBounds(entry?.bounds);
    const width = bounds ? bounds.w : 0;
    if (entry?.instanceId) {
      placements.push({ instanceId: entry.instanceId, x: cursorX, y: baseY });
    }
    const spacing = idx < entries.length - 1 ? gaps?.[idx] : 0;
    cursorX += width + Math.max(resolvedMinGap, readNumber(spacing, resolvedMinGap));
  }
  return placements;
}

/**
 * Insert between.
 */
export function insertBetween(
  order: readonly LayoutEntry[] | null | undefined,
  _leftId: LayoutEntry | GraphId | null | undefined,
  rightId: LayoutEntry | GraphId | null | undefined,
  newBounds: unknown,
  gap: unknown,
): Map<string, MoveDelta> {
  const entries = Array.isArray(order) ? order : [];
  const target = resolveEntry(entries, rightId);
  const shift = readNumber(isRecord(newBounds) ? newBounds.w : undefined, 0) + readNumber(gap, 0);
  const moves = new Map<string, MoveDelta>();
  if (!target || !shift) {
    return moves;
  }
  const targetBounds = normalizeBounds(target.bounds);
  if (!targetBounds) {
    return moves;
  }
  for (const entry of entries) {
    const bounds = normalizeBounds(entry?.bounds);
    if (!bounds || !entry?.instanceId) {
      continue;
    }
    if (bounds.x >= targetBounds.x) {
      moves.set(entry.instanceId, { dx: shift, dy: 0 });
    }
  }
  return moves;
}

/**
 * Insert before.
 */
export function insertBefore(
  order: readonly LayoutEntry[] | null | undefined,
  targetId: LayoutEntry | GraphId | null | undefined,
  newBounds: unknown,
  gap: unknown,
): Map<string, MoveDelta> {
  return insertBetween(order, null, targetId, newBounds, gap);
}

/**
 * Append after.
 */
export function appendAfter(
  order: readonly LayoutEntry[] | null | undefined,
  lastId: LayoutEntry | GraphId | null | undefined,
  _newBounds: unknown,
  gap: unknown,
): LayoutPoint {
  const entries = Array.isArray(order) ? order : [];
  const lastEntry = resolveEntry(entries, lastId);
  const lastBounds = normalizeBounds(lastEntry?.bounds);
  const spacing = readNumber(gap, 0);
  if (!lastBounds) {
    return { x: spacing, y: 0 };
  }
  return { x: lastBounds.x + lastBounds.w + spacing, y: lastBounds.y };
}

/**
 * Swap order.
 */
export function swapOrder(
  order: readonly LayoutEntry[] | null | undefined,
  aId: LayoutEntry | GraphId | null | undefined,
  bId: LayoutEntry | GraphId | null | undefined,
  options: SwapLayoutOptions = {},
): LayoutPlacement[] {
  const entries = Array.isArray(order) ? order.slice() : [];
  const resolvedOrigin = Array.isArray(options?.origin) ? options.origin : null;
  const resolvedMinGap = readNumber(options?.minGap, 0);
  const gapSequence = buildGapSequence(entries, resolvedMinGap, options?.gap, options?.gaps);
  const aEntry = resolveEntry(entries, aId);
  const bEntry = resolveEntry(entries, bId);
  if (!aEntry || !bEntry) {
    return [];
  }
  const idxA = entries.indexOf(aEntry);
  const idxB = entries.indexOf(bEntry);
  if (idxA < 0 || idxB < 0 || idxA === idxB) {
    return [];
  }
  [entries[idxA], entries[idxB]] = [entries[idxB], entries[idxA]];
  return layoutFromOrderWithGaps(entries, resolvedOrigin, gapSequence, resolvedMinGap);
}

/**
 * Replace cube.
 */
export function replaceCube(
  order: readonly LayoutEntry[] | null | undefined,
  targetId: LayoutEntry | GraphId | null | undefined,
  newBounds: unknown,
  _gap: unknown,
): Map<string, MoveDelta> {
  const entries = Array.isArray(order) ? order : [];
  const target = resolveEntry(entries, targetId);
  const targetBounds = normalizeBounds(target?.bounds);
  const moves = new Map<string, MoveDelta>();
  if (!target || !targetBounds) {
    return moves;
  }
  const delta =
    readNumber(isRecord(newBounds) ? newBounds.w : undefined, targetBounds.w) - targetBounds.w;
  if (!delta) {
    return moves;
  }
  for (const entry of entries) {
    if (!entry?.instanceId || entry.instanceId === target.instanceId) {
      continue;
    }
    const bounds = normalizeBounds(entry.bounds);
    if (!bounds) {
      continue;
    }
    if (delta > 0 && bounds.x > targetBounds.x) {
      moves.set(entry.instanceId, { dx: delta, dy: 0 });
    }
    if (delta < 0 && bounds.x < targetBounds.x) {
      moves.set(entry.instanceId, { dx: delta, dy: 0 });
    }
  }
  return moves;
}
