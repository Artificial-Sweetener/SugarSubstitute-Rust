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
 * Own the SugarCubes cosmetic snapshotting layer in
 * `frontend/comfyui/ui/graph/CosmeticSnapshotter.js`.
 */

import { snapshotGroup, snapshotInstance } from './DirtySnapshotter.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type { ComfyGraph, ComfyGroup, GraphId } from '../types/graph.js';

interface CosmeticNodeSnapshot {
  id: unknown;
  title: unknown;
  pos: unknown[] | null;
  size: unknown[] | null;
  flags: unknown;
}

export interface CosmeticInstanceSnapshot {
  nodes: CosmeticNodeSnapshot[];
  group: unknown;
}

function normalizeCosmeticNode(node: unknown): CosmeticNodeSnapshot {
  const record = isRecord(node) ? node : {};
  return {
    id: record.id || '',
    title: record.title || '',
    pos: Array.isArray(record.pos) ? record.pos : null,
    size: Array.isArray(record.size) ? record.size : null,
    flags: record.flags ?? null,
  };
}

/**
 * Snapshot runtime cosmetic state for one managed cube instance.
 */
export function snapshotCosmeticInstance(
  graph: ComfyGraph | null | undefined,
  nodeIds: readonly GraphId[],
  markerIds: readonly GraphId[],
  anchor: Vec2 | null | undefined,
  group: ComfyGroup | null | undefined,
): CosmeticInstanceSnapshot {
  const groupSnapshot = snapshotGroup(group, false);
  const rawPayload: unknown = snapshotInstance(graph, nodeIds, markerIds, anchor, groupSnapshot, {
    useSymbols: false,
    useInputNames: false,
    stripSugarcubesProperties: true,
  });
  const payload: UnknownRecord = isRecord(rawPayload) ? rawPayload : {};
  const nodes = Array.isArray(payload.nodes)
    ? payload.nodes.map((node: unknown) => normalizeCosmeticNode(node))
    : [];
  nodes.sort((left, right) => String(left.id).localeCompare(String(right.id)));
  return {
    nodes,
    group: payload.group || null,
  };
}
