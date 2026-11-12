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
/** Translate native projected field ownership and numeric constraints into portable dimension inputs. */
import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';
import { getGraphNodes } from '../../graph/GraphQuery.js';
import type { ComfyNode } from '../../types/graph.js';
import { NativeSubgraphBoundaryResolver } from '../../cube/graph/NativeSubgraphBoundaryResolver.js';
import type { CanvasDimensionLimits } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { DimensionFieldTarget } from '../application/CanvasDimensionFields.js';

/** Read the native integer field grid, preferring actual step2 over LiteGraph's tenfold drag step. */
export function nativeDimensionLimits(widget: UnknownRecord): CanvasDimensionLimits {
  const options = isRecord(widget.options) ? widget.options : {};
  const spec = Array.isArray(widget.spec) && isRecord(widget.spec[1]) ? widget.spec[1] : {};
  const number = (value: unknown, fallback: number): number =>
    typeof value === 'number' && Number.isFinite(value) ? value : fallback;
  const step = number(options.step2, number(spec.step, 1));
  return {
    minimum: Math.max(1, Math.ceil(number(options.min, number(spec.min, 1)))),
    maximum: Math.min(300000, Math.floor(number(options.max, number(spec.max, 300000)))),
    multiple: Number.isSafeInteger(step) && step > 0 ? step : 1,
  };
}

/** Resolve exact model owners only during an explicit probe command, never while discovering DOM surfaces. */
export function nativeDimensionTargets(
  widget: UnknownRecord,
  graph: unknown,
): readonly DimensionFieldTarget[] {
  const owner = widget.sugarcubesNativeWidgetOwner;
  if (typeof owner !== 'function') return [];
  const source: unknown = owner();
  if (!isRecord(source) || typeof source.name !== 'string') return [];
  const visited = new Set<object>();
  const find = (
    current: unknown,
    prefix: readonly string[],
  ): { node: ComfyNode; prefix: readonly string[] } | null => {
    if (!isRecord(current) || visited.has(current)) return null;
    visited.add(current);
    for (const node of getGraphNodes(current)) {
      if (node.id == null) continue;
      if (node === source.node) return { node, prefix };
      if (node.subgraph) {
        const result = find(node.subgraph, [...prefix, String(node.id)]);
        if (result) return result;
      }
    }
    return null;
  };
  const found = find(graph, []);
  if (!found) return [];
  if (found.node.isSubgraphNode?.() === true) {
    const resolver = new NativeSubgraphBoundaryResolver(console);
    return resolver.resolveWidgetTargets(found.node, source.name).map((target) => ({
      nodeId: [...found.prefix, String(target.nodeId)].join(':'),
      inputName: target.inputName,
    }));
  }
  return [{ nodeId: [...found.prefix, String(found.node.id)].join(':'), inputName: source.name }];
}
