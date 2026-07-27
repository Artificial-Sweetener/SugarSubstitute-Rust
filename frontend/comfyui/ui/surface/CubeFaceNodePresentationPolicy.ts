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
/** Own transient native-node state used only while presenting Cube-face cards. */

import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';

interface RememberedNodeProperty {
  key: 'flags' | 'widgets_start_y' | 'widgets_up';
  owned: boolean;
  value: unknown;
}

/** Reserve one normal LiteGraph slot row above slot-suppressed Cube-face widgets. */
export const CUBE_FACE_WIDGET_GUTTER = 20;
const NATIVE_WIDGET_INSET = 2;

/** Create face flags that retain native state except graph collapse. */
function createCubeFaceFlags(flags: unknown): UnknownRecord {
  return {
    ...(isRecord(flags) ? flags : {}),
    collapsed: false,
  };
}

/** Create safe Comfy Vue render data without any internal graph boundary slots. */
export function createCubeFaceNodeData(nodeData: UnknownRecord): UnknownRecord {
  return {
    ...nodeData,
    flags: createCubeFaceFlags(nodeData.flags),
    inputs: [],
    outputs: [],
  };
}

/** Expose expanded slotless state during one native operation, then restore exact ownership. */
export function withCubeFaceNodePresentation<Result>(
  node: ComfyNode,
  operation: () => Result,
): Result {
  const remembered = rememberNodeProperties(node, ['flags', 'widgets_start_y', 'widgets_up']);
  node.flags = createCubeFaceFlags(node.flags);
  Reflect.set(node, 'widgets_up', true);
  // Preserve the first native slot row as spacing when Cube cards suppress their graph slots.
  Reflect.set(node, 'widgets_start_y', CUBE_FACE_WIDGET_GUTTER + NATIVE_WIDGET_INSET);
  try {
    return operation();
  } finally {
    restoreNodeProperties(node, remembered);
  }
}

/** Capture exact own-property semantics for every temporary face override. */
function rememberNodeProperties(
  node: ComfyNode,
  keys: readonly RememberedNodeProperty['key'][],
): RememberedNodeProperty[] {
  return keys.map((key) => ({
    key,
    owned: Object.prototype.hasOwnProperty.call(node, key),
    value: node[key],
  }));
}

/** Restore temporary face properties without manufacturing inherited state. */
function restoreNodeProperties(
  node: ComfyNode,
  remembered: readonly RememberedNodeProperty[],
): void {
  for (const property of remembered) {
    Reflect.set(node, property.key, property.value);
    if (!property.owned) Reflect.deleteProperty(node, property.key);
  }
}

/** Return whether Comfy will present at least one real widget control. */
export function cubeFaceNodeHasVisibleWidgets(node: ComfyNode): boolean {
  const widgets = node.widgets ?? [];
  if (widgets.length === 0) return false;
  const isWidgetVisible = node.isWidgetVisible;
  if (typeof isWidgetVisible !== 'function') return true;
  return widgets.some((widget) => isWidgetVisible.call(node, widget) !== false);
}
