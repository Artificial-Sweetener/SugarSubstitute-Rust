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
/** Carry native field ownership through transient Vue widget projections. */
import { NativeSubgraphBoundaryResolver } from '../cube/graph/NativeSubgraphBoundaryResolver.js';
import type { NativeWidgetOrigin } from '../cube/graph/NativeSubgraphBoundaryResolver.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';

const ORIGINS_PROPERTY = 'sugarcubesNativeWidgetOrigins';

/** Annotate projected subgraph widgets without mutating persistent host controls. */
export function projectNativeWidgetOrigins(
  node: ComfyNode,
  widgets: readonly unknown[],
): unknown[] {
  if (typeof node.isSubgraphNode !== 'function' || node.isSubgraphNode() !== true) {
    return [...widgets];
  }
  const resolver = new NativeSubgraphBoundaryResolver(console);
  return widgets.map((widget) =>
    isRecord(widget) && typeof widget.name === 'string'
      ? { ...widget, [ORIGINS_PROPERTY]: resolver.resolveWidgetOrigins(node, widget.name) }
      : widget,
  );
}

/** Distinguish absent projection metadata from malformed or unresolved native ownership. */
export function readNativeWidgetOrigins(
  widget: UnknownRecord,
): readonly NativeWidgetOrigin[] | null {
  if (!Object.prototype.hasOwnProperty.call(widget, ORIGINS_PROPERTY)) return null;
  const origins: unknown = widget[ORIGINS_PROPERTY];
  if (!Array.isArray(origins)) return [];
  const result: NativeWidgetOrigin[] = [];
  for (const origin of origins) {
    if (
      !isRecord(origin) ||
      typeof origin.classType !== 'string' ||
      !origin.classType.trim() ||
      typeof origin.inputName !== 'string' ||
      !origin.inputName.trim()
    )
      return [];
    result.push({ classType: origin.classType, inputName: origin.inputName });
  }
  return result;
}
