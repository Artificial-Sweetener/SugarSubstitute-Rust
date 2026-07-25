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
/** Swap first-class Cube nodes without depending on legacy group chrome state. */

import type {
  CubeFaceChromeMetadata,
  CubeSwapDirection,
} from '../../surface/CubeFaceChromeActions.js';
import type { ComfyGraph } from '../../types/graph.js';
import type { CubeNode } from './ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from './CubeNodeCatalog.js';
import type { LiteGraphCubeNodeInteractionHistory } from '../../surface/ComfyLiteGraphCubeNodeInteraction.js';

interface CubeNodeSwapCoordinatorOptions {
  graph: ComfyGraph;
  nodes: CubeNodeCatalog;
  history: LiteGraphCubeNodeInteractionHistory;
  setDirtyCanvas?(foreground?: boolean, background?: boolean): void;
}

interface SwapPlan {
  current: CubeNode;
  neighbor: CubeNode;
}

/** Own node-based Cube swap eligibility and state transitions. */
export class CubeNodeSwapCoordinator {
  readonly #graph: ComfyGraph;
  readonly #nodes: CubeNodeCatalog;
  readonly #history: LiteGraphCubeNodeInteractionHistory;
  readonly #setDirtyCanvas: (foreground?: boolean, background?: boolean) => void;

  /** Bind graph-owned Cube nodes and the native history seam. */
  constructor(options: CubeNodeSwapCoordinatorOptions) {
    this.#graph = options.graph;
    this.#nodes = options.nodes;
    this.#history = options.history;
    this.#setDirtyCanvas = options.setDirtyCanvas ?? (() => undefined);
  }

  /** Return whether a neighboring first-class Cube node can be swapped in that direction. */
  canSwap(metadata: CubeFaceChromeMetadata, direction: CubeSwapDirection): boolean {
    return this.#resolvePlan(metadata, direction) !== null;
  }

  /** Swap this Cube node with its nearest eligible first-class Cube neighbor. */
  swap(metadata: CubeFaceChromeMetadata, direction: CubeSwapDirection): void {
    const plan = this.#resolvePlan(metadata, direction);
    if (!plan) return;
    const currentPosition = readPosition(plan.current);
    const neighborPosition = readPosition(plan.neighbor);
    this.#history.beforeChange?.();
    writePosition(plan.current, neighborPosition);
    writePosition(plan.neighbor, currentPosition);
    this.#graph.afterChange?.();
    this.#graph.setDirtyCanvas?.(true, true);
    this.#setDirtyCanvas(true, true);
    this.#nodes.changed(plan.current);
    this.#nodes.changed(plan.neighbor);
    this.#history.afterChange?.();
  }

  /** Resolve the nearest eligible Cube node using current node positions. */
  #resolvePlan(metadata: CubeFaceChromeMetadata, direction: CubeSwapDirection): SwapPlan | null {
    const instanceId = typeof metadata.instance_id === 'string' ? metadata.instance_id.trim() : '';
    if (!instanceId) return null;
    const current = this.#nodes.get(instanceId);
    if (!current) return null;
    const ordered = [...this.#nodes.list()].sort(compareCubeNodePosition);
    const index = ordered.indexOf(current);
    if (index < 0) return null;
    const neighbor = ordered[index + (direction === 'left' ? -1 : 1)] ?? null;
    return neighbor ? { current, neighbor } : null;
  }
}

/** Sort left-to-right with a vertical tie-breaker for stable row-independent swaps. */
function compareCubeNodePosition(left: CubeNode, right: CubeNode): number {
  const leftPosition = readPosition(left);
  const rightPosition = readPosition(right);
  return leftPosition[0] - rightPosition[0] || leftPosition[1] - rightPosition[1];
}

/** Read a native Cube node position as finite graph-space coordinates. */
function readPosition(node: CubeNode): [number, number] {
  return [finite(node.pos[0]), finite(node.pos[1])];
}

/** Write a native Cube node position through its native API when present. */
function writePosition(node: CubeNode, position: readonly [number, number]): void {
  if (typeof node.setPos === 'function') {
    node.setPos(position[0], position[1]);
  }
  node.pos[0] = position[0];
  node.pos[1] = position[1];
}

/** Prevent invalid persisted graph coordinates from entering swap math. */
function finite(value: unknown): number {
  const numberValue = Number(value);
  return Number.isFinite(numberValue) ? numberValue : 0;
}
