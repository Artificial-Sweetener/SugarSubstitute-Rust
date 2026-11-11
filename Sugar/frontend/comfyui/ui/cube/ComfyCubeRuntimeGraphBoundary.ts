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
/** Validate native graph construction capabilities before they enter Cube orchestration. */
import { isRecord, type UnknownRecord } from '../types/common.js';
import type { NativeCubeSubgraph, NativeGraphNode } from './ComfyCubeGraphBuilder.js';
import type { LiteGraphCubeNodeCanvas } from '../surface/ComfyLiteGraphCubeNodeHost.js';
import type { ConnectableNode } from './migration/LegacyCubeMigrationCoordinator.js';

export interface CubeRuntimeGraph {
  subgraphs: Map<string, NativeCubeSubgraph>;
  _nodes?: unknown[];
  onConfigure?: ((data: UnknownRecord) => void) | null;
  onSerialize?: ((data: UnknownRecord) => void) | null;
  add(node: NativeGraphNode): void;
  remove(node: NativeGraphNode): void;
  getLink(id: string | number): import('../types/graph.js').ComfyLink | null;
  getNodeById(id: string | number): unknown;
}

/** Validate the node surface required by native Cube graph construction. */
export function isNativeGraphNode(value: unknown): value is NativeGraphNode {
  if (!isRecord(value)) return false;
  value.properties = isRecord(value.properties) ? value.properties : {};
  return (
    isNumericVector(value.pos) &&
    isNumericVector(value.size) &&
    Array.isArray(value.inputs) &&
    Array.isArray(value.outputs) &&
    typeof value.connect === 'function'
  );
}

/** Validate the Comfy subgraph API used by Cube assembly and serialization. */
export function isNativeCubeSubgraph(value: unknown): value is NativeCubeSubgraph {
  return (
    isRecord(value) &&
    typeof value.id === 'string' &&
    typeof value.name === 'string' &&
    Array.isArray(value._nodes) &&
    Array.isArray(value.inputs) &&
    Array.isArray(value.outputs) &&
    isRecord(value.inputNode) &&
    isRecord(value.outputNode) &&
    typeof value.add === 'function' &&
    typeof value.remove === 'function' &&
    typeof value.addInput === 'function' &&
    typeof value.addOutput === 'function' &&
    typeof value.configure === 'function'
  );
}

/** Require Comfy's native definition map and lifecycle hook surface. */
export function requireRuntimeGraph(graph: UnknownRecord): CubeRuntimeGraph {
  if (!(graph.subgraphs instanceof Map)) {
    throw new TypeError('LGraph.subgraphs is unavailable.');
  }
  return graph as unknown as CubeRuntimeGraph;
}

/** Accept LiteGraph's array and typed-array geometry vectors. */
export function isNumericVector(value: unknown): value is number[] | Float32Array | Float64Array {
  return Array.isArray(value) || value instanceof Float32Array || value instanceof Float64Array;
}

/** Require the exact host capabilities used by the Nodes 1.0 Cube node face. */
export function requireLiteGraphCubeNodeCanvas(canvas: UnknownRecord): LiteGraphCubeNodeCanvas {
  if (
    !(canvas.canvas instanceof HTMLCanvasElement) ||
    typeof canvas.drawNode !== 'function' ||
    typeof canvas.processWidgetClick !== 'function' ||
    !isNumericVector(canvas.graph_mouse)
  ) {
    throw new TypeError('Comfy Nodes 1.0 Cube-node integration is unavailable.');
  }
  return canvas as unknown as LiteGraphCubeNodeCanvas;
}

/** Validate a native node capable of owning one restored graph connection. */
export function isConnectableNode(value: unknown): value is ConnectableNode {
  return (
    isRecord(value) &&
    Array.isArray(value.inputs) &&
    Array.isArray(value.outputs) &&
    typeof value.connect === 'function'
  );
}
