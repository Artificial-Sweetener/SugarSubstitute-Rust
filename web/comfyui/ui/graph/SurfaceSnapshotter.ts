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
 * Own the SugarCubes surface snapshotting layer in
 * `web/comfyui/ui/graph/SurfaceSnapshotter.js`.
 */

import { readWidgetValue } from './Markers.js';
import { buildSurfaceNodesBySymbol } from './SurfaceNodeResolver.js';
import { trackedSurfaceControls } from '../core/SurfaceValuePolicy.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyGraph, ComfyNode, CubeSurface, GraphId } from '../types/graph.js';

export interface SurfaceValueSnapshotEntry {
  control_id: string;
  value: unknown;
}

export interface SurfaceValueSnapshot {
  controls: SurfaceValueSnapshotEntry[];
}

function readPropertyValue(node: ComfyNode | null | undefined, inputName: string): unknown {
  if (!node || typeof inputName !== 'string') {
    return null;
  }
  if (node.properties && Object.prototype.hasOwnProperty.call(node.properties, inputName)) {
    return node.properties[inputName];
  }
  return null;
}

/**
 * Build a stable surface-value snapshot from current live nodes.
 */
export function snapshotSurfaceInstance(
  graph: ComfyGraph | null | undefined,
  nodeIds: readonly GraphId[] | null | undefined,
  surface: CubeSurface | null | undefined,
): SurfaceValueSnapshot {
  const controls = trackedSurfaceControls(surface);
  if (!controls.length) {
    return { controls: [] };
  }
  const nodesBySymbol = buildSurfaceNodesBySymbol(graph, nodeIds, surface);
  const values: SurfaceValueSnapshotEntry[] = [];
  for (const control of controls) {
    const controlId = typeof control?.control_id === 'string' ? control.control_id.trim() : '';
    const symbol = typeof control?.symbol === 'string' ? control.symbol.trim() : '';
    const inputName = typeof control?.input_name === 'string' ? control.input_name.trim() : '';
    if (!controlId || !symbol || !inputName) {
      continue;
    }
    const node = nodesBySymbol.get(symbol);
    if (!node) {
      values.push({ control_id: controlId, value: null });
      continue;
    }
    const widgetValue = readWidgetValue(node, inputName);
    const value =
      widgetValue !== '' || readPropertyValue(node, inputName) == null
        ? widgetValue
        : readPropertyValue(node, inputName);
    values.push({ control_id: controlId, value });
  }
  return { controls: values };
}

/**
 * Build a stable surface-value snapshot from persisted flavor values.
 */
export function snapshotSurfaceValues(
  surface: CubeSurface | null | undefined,
  values: unknown,
): SurfaceValueSnapshot {
  const controls = trackedSurfaceControls(surface);
  const lookup: UnknownRecord = isRecord(values) ? values : {};
  const entries: SurfaceValueSnapshotEntry[] = [];
  for (const control of controls) {
    const controlId = typeof control?.control_id === 'string' ? control.control_id.trim() : '';
    if (!controlId) {
      continue;
    }
    entries.push({
      control_id: controlId,
      value: Object.prototype.hasOwnProperty.call(lookup, controlId) ? lookup[controlId] : null,
    });
  }
  return { controls: entries };
}
