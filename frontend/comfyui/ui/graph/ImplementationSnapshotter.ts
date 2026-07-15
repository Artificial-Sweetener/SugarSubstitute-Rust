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
 * Own the SugarCubes implementation snapshotting layer in
 * `frontend/comfyui/ui/graph/ImplementationSnapshotter.js`.
 */

import { buildDefinitionSnapshot, snapshotInstance } from './DirtySnapshotter.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type { ComfyGraph, CubeSurface, GraphId } from '../types/graph.js';

interface ImplementationNodeSnapshot {
  id: unknown;
  type: unknown;
  mode: number | null;
  properties: unknown;
  widgets: unknown[];
}

export interface ImplementationSnapshot {
  nodes: ImplementationNodeSnapshot[];
  links: unknown[];
}

function buildSurfaceControlMap(surface: CubeSurface | null | undefined): Map<string, Set<string>> {
  const controls = Array.isArray(surface?.controls) ? surface.controls : [];
  const controlMap = new Map<string, Set<string>>();
  for (const control of controls) {
    const symbol = typeof control?.symbol === 'string' ? control.symbol.trim() : '';
    const inputName = typeof control?.input_name === 'string' ? control.input_name.trim() : '';
    if (!symbol || !inputName) {
      continue;
    }
    const inputSet = controlMap.get(symbol) || new Set();
    inputSet.add(inputName);
    controlMap.set(symbol, inputSet);
  }
  return controlMap;
}

function stripCosmeticFields(node: UnknownRecord): ImplementationNodeSnapshot {
  return {
    id: node?.id || '',
    type: node?.type || '',
    mode:
      typeof node.mode === 'number' && Number.isInteger(node.mode) && node.mode > 0
        ? node.mode
        : null,
    properties: node?.properties ?? null,
    widgets: Array.isArray(node?.widgets) ? node.widgets : [],
  };
}

function filterFaceWidgets(
  node: UnknownRecord,
  surfaceControlMap: ReadonlyMap<string, ReadonlySet<string>>,
): UnknownRecord {
  const properties = isRecord(node.properties) ? node.properties : {};
  const symbol =
    typeof properties.sugarcubes_symbol === 'string' ? properties.sugarcubes_symbol.trim() : '';
  const inputNames =
    surfaceControlMap.get(String(node?.id || '')) ||
    (symbol ? surfaceControlMap.get(symbol) : null);
  if (!inputNames || !Array.isArray(node?.widgets)) {
    return node;
  }
  return {
    ...node,
    widgets: node.widgets.filter((widget: unknown) => {
      const name = isRecord(widget) && typeof widget.name === 'string' ? widget.name : '';
      return !inputNames.has(name);
    }),
  };
}

function normalizeImplementationPayload(
  payload: unknown,
  surface: CubeSurface | null | undefined,
): ImplementationSnapshot {
  const record = isRecord(payload) ? payload : {};
  const surfaceControlMap = buildSurfaceControlMap(surface);
  const nodes = Array.isArray(record.nodes)
    ? record.nodes
        .filter(isRecord)
        .map((node: UnknownRecord) => filterFaceWidgets(node, surfaceControlMap))
        .map((node) => stripCosmeticFields(node))
    : [];
  nodes.sort((left, right) => String(left.id).localeCompare(String(right.id)));
  const links = Array.isArray(record.links) ? record.links : [];
  return { nodes, links };
}

/**
 * Snapshot runtime implementation state for one managed cube instance.
 */
export function snapshotImplementationInstance(
  graph: ComfyGraph | null | undefined,
  nodeIds: readonly GraphId[],
  markerIds: readonly GraphId[],
  anchor: Vec2 | null | undefined,
  surface: CubeSurface | null | undefined,
  options: UnknownRecord = {},
): ImplementationSnapshot {
  const payload: unknown = snapshotInstance(graph, nodeIds, markerIds, anchor, null, options);
  return normalizeImplementationPayload(payload, surface);
}

/**
 * Snapshot imported definition state for implementation diffing.
 */
export function snapshotImplementationDefinition(
  definition: unknown,
  surface: CubeSurface | null = null,
): ImplementationSnapshot {
  const payload: unknown = buildDefinitionSnapshot(definition);
  return normalizeImplementationPayload(payload, surface);
}
