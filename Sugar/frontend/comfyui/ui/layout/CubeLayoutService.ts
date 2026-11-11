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
 * Own the SugarCubes layout orchestration layer in `frontend/comfyui/ui/layout/CubeLayoutService.js`.
 */

import { CubeInstanceIndex } from './CubeInstanceIndex.js';
import {
  appendAfter,
  deriveChainOrder,
  insertBefore,
  insertBetween,
  replaceCube,
  swapOrder,
} from './CubeLayoutEngine.js';
import { applyMoves } from './CubeMover.js';
import type {
  ChainOrderStrategy,
  LayoutEntry,
  LayoutIndex,
  LayoutPlacement,
  LayoutPoint,
  SwapLayoutOptions,
} from './CubeLayoutEngine.js';
import type { MoveDelta, MovableCubeInstance } from './CubeMover.js';
import type { CubeInstance } from '../graph/InstanceBuilder.js';
import type { ComfyApplication, ComfyGraph } from '../types/graph.js';

const LAYOUT_MOVE_OPTIONS = Object.freeze({ recomputeBounds: false });

interface LayoutAdapter {
  getApp?(): ComfyApplication | null;
}

interface RefreshCoordinator {
  scheduleRefresh?(options: { graph: ComfyGraph | null; reason: string }): unknown;
}

interface DirtyRefreshCoordinator {
  requestRefresh?(options: { graph: ComfyGraph | null; reason: string }): unknown;
}

interface InstanceBuilderTarget {
  build(graph: ComfyGraph | null | undefined): CubeInstance[];
}

interface LayoutMover {
  applyMoves(
    graph: ComfyGraph | null | undefined,
    index: { instances?: MovableCubeInstance[] } | null | undefined,
    moves: ReadonlyMap<string, MoveDelta> | null | undefined,
    options?: { recomputeBounds?: boolean },
  ): void;
}

export interface LayoutServiceIndex extends LayoutIndex {
  instances: Array<LayoutEntry & MovableCubeInstance>;
  instanceById: Map<string, LayoutEntry & MovableCubeInstance>;
}

type IndexFactory = (graph: ComfyGraph | null) => LayoutServiceIndex;

interface LayoutServiceOptions {
  adapter?: LayoutAdapter | null;
  instanceManager?: RefreshCoordinator | null;
  dirtyManager?: DirtyRefreshCoordinator | null;
  instanceBuilder?: InstanceBuilderTarget | null;
  mover?: LayoutMover | null;
  indexFactory?: IndexFactory | null;
}

interface LayoutStrategyOptions extends ChainOrderStrategy {
  strategy?: ChainOrderStrategy | null | undefined;
}

interface BaseLayoutRequest {
  graph?: ComfyGraph | null;
  gap?: unknown;
  newBounds?: unknown;
  reason?: string;
  strategy?: ChainOrderStrategy | null;
}

interface AppendRequest extends BaseLayoutRequest {
  lastId?: string | number | LayoutEntry | null;
}

interface InsertBetweenRequest extends BaseLayoutRequest {
  leftId?: string | number | LayoutEntry | null;
  rightId?: string | number | LayoutEntry | null;
}

interface InsertBeforeRequest extends BaseLayoutRequest {
  targetId?: string | number | LayoutEntry | null;
}

interface SwapRequest extends BaseLayoutRequest {
  aId?: string | number | LayoutEntry | null;
  bId?: string | number | LayoutEntry | null;
  order?: LayoutEntry[] | null;
  layout?: SwapLayoutOptions;
}

interface ReplaceRequest extends BaseLayoutRequest {
  targetId?: string | number | LayoutEntry | null;
}

function readNumber(value: unknown, fallback = 0): number {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function resolveGraph(
  adapter: LayoutAdapter | null,
  graph: ComfyGraph | null | undefined,
): ComfyGraph | null {
  if (graph) {
    return graph;
  }
  return adapter?.getApp?.()?.graph || null;
}

function buildMovesFromPlacements(
  placements: readonly LayoutPlacement[],
  index: LayoutServiceIndex,
): Map<string, MoveDelta> {
  const moves = new Map<string, MoveDelta>();
  for (const placement of placements || []) {
    const instanceId = placement?.instanceId;
    if (!instanceId) {
      continue;
    }
    const entry = index?.instanceById?.get?.(instanceId) || null;
    const bounds = entry?.bounds || null;
    if (!bounds) {
      continue;
    }
    const dx = readNumber(placement.x, bounds.x) - readNumber(bounds.x, 0);
    const dy = readNumber(placement.y, bounds.y) - readNumber(bounds.y, 0);
    if (!dx && !dy) {
      continue;
    }
    moves.set(instanceId, { dx, dy });
  }
  return moves;
}

/**
 * Coordinate cube layout service behavior for the SugarCubes UI.
 */
export class CubeLayoutService {
  private readonly adapter: LayoutAdapter | null;
  private readonly instanceManager: RefreshCoordinator | null;
  private readonly dirtyManager: DirtyRefreshCoordinator | null;
  private readonly instanceBuilder: InstanceBuilderTarget | null;
  private readonly mover: LayoutMover;
  private readonly indexFactory: IndexFactory | null;

  constructor({
    adapter,
    instanceManager,
    dirtyManager,
    instanceBuilder,
    mover,
    indexFactory,
  }: LayoutServiceOptions = {}) {
    this.adapter = adapter || null;
    this.instanceManager = instanceManager || null;
    this.dirtyManager = dirtyManager || null;
    this.instanceBuilder = instanceBuilder || null;
    this.mover = mover || { applyMoves };
    this.indexFactory = typeof indexFactory === 'function' ? indexFactory : null;
  }

  buildIndex(graph: ComfyGraph | null): LayoutServiceIndex {
    if (this.indexFactory) {
      return this.indexFactory(graph);
    }
    return new CubeInstanceIndex({
      graph,
      ...(this.instanceBuilder ? { instanceBuilder: this.instanceBuilder } : {}),
    });
  }

  deriveOrder(index: LayoutIndex, options: LayoutStrategyOptions = {}): LayoutEntry[] {
    const direct =
      options && typeof options === 'object' && !Array.isArray(options) ? { ...options } : {};
    const nested = options.strategy ?? null;
    const strategy = nested ? { ...nested, ...direct } : direct;
    delete strategy.strategy;
    return deriveChainOrder(index, strategy);
  }

  applyMovePlan(
    graph: ComfyGraph | null,
    index: LayoutServiceIndex,
    moves: Map<string, MoveDelta>,
    reason: string,
  ): void {
    if (!(moves instanceof Map) || !moves.size) {
      return;
    }
    this.mover?.applyMoves?.(graph, index, moves, LAYOUT_MOVE_OPTIONS);
    graph?.afterChange?.();
    graph?.setDirtyCanvas?.(true, true);
    this.instanceManager?.scheduleRefresh?.({ graph, reason });
    this.dirtyManager?.requestRefresh?.({ graph, reason });
  }

  appendCube({
    graph,
    lastId,
    newBounds,
    gap,
    reason = 'layout-append',
    strategy,
  }: AppendRequest = {}): {
    placement: LayoutPoint;
    order: LayoutEntry[];
    index: LayoutServiceIndex;
    reason: string;
  } {
    const targetGraph = resolveGraph(this.adapter, graph);
    const index = this.buildIndex(targetGraph);
    const order = this.deriveOrder(index, { graph: targetGraph, strategy });
    const placement = appendAfter(order, lastId, newBounds, gap);
    return { placement, order, index, reason };
  }

  insertBetween({
    graph,
    leftId,
    rightId,
    newBounds,
    gap,
    reason = 'layout-insert-between',
    strategy,
  }: InsertBetweenRequest = {}) {
    const targetGraph = resolveGraph(this.adapter, graph);
    const index = this.buildIndex(targetGraph);
    const order = this.deriveOrder(index, { graph: targetGraph, strategy });
    const moves = insertBetween(order, leftId, rightId, newBounds, gap);
    this.applyMovePlan(targetGraph, index, moves, reason);
    return { moves, order, index, reason };
  }

  insertBefore({
    graph,
    targetId,
    newBounds,
    gap,
    reason = 'layout-insert-before',
    strategy,
  }: InsertBeforeRequest = {}) {
    const targetGraph = resolveGraph(this.adapter, graph);
    const index = this.buildIndex(targetGraph);
    const order = this.deriveOrder(index, { graph: targetGraph, strategy });
    const moves = insertBefore(order, targetId, newBounds, gap);
    this.applyMovePlan(targetGraph, index, moves, reason);
    return { moves, order, index, reason };
  }

  swapOrder({
    graph,
    aId,
    bId,
    order,
    layout,
    reason = 'layout-swap',
    strategy,
  }: SwapRequest = {}) {
    const targetGraph = resolveGraph(this.adapter, graph);
    const index = this.buildIndex(targetGraph);
    const resolvedOrder = Array.isArray(order)
      ? order
      : this.deriveOrder(index, { graph: targetGraph, strategy });
    const placements = swapOrder(resolvedOrder, aId, bId, layout ?? {});
    const moves = buildMovesFromPlacements(placements, index);
    this.applyMovePlan(targetGraph, index, moves, reason);
    return { placements, moves, order: resolvedOrder, index, reason };
  }

  replaceCube({
    graph,
    targetId,
    newBounds,
    gap,
    reason = 'layout-replace',
    strategy,
  }: ReplaceRequest = {}) {
    const targetGraph = resolveGraph(this.adapter, graph);
    const index = this.buildIndex(targetGraph);
    const order = this.deriveOrder(index, { graph: targetGraph, strategy });
    const moves = replaceCube(order, targetId, newBounds, gap);
    this.applyMovePlan(targetGraph, index, moves, reason);
    return { moves, order, index, reason };
  }
}
