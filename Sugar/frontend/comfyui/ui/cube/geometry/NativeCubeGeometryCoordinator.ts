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
/** Restore renderer-safe internal layouts when a native Cube editor mounts. */

import type { CubeNode } from '../node/ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from '../node/CubeNodeCatalog.js';
import { applyMeasuredNodeGeometry } from '../../geometry/ComfyInstanceGeometry.js';
import { hasMountedNodePresentation } from '../../geometry/ComfyNodeGeometry.js';
import type { NodeRenderer } from '../../geometry/RendererGeometryPolicy.js';
import type { CanvasGraphChangeSource } from '../../surface/ComfyCanvasGraphChangeAdapter.js';
import { isRecord } from '../../types/common.js';
import type { ComfyNode } from '../../types/graph.js';
import { readNativeCubeAuthoredLayout } from './NativeCubeAuthoredLayout.js';
import { applyNativeCubeBoundaryLayout } from './NativeCubeBoundaryLayout.js';

const MAX_MOUNT_ATTEMPTS = 20;
const MOUNT_RETRY_DELAY_MS = 50;
const VERIFICATION_DELAYS_MS = [150, 500] as const;

export interface NativeCubeGeometryScheduler {
  schedule(callback: () => void, delayMs: number): unknown;
  cancel(handle: unknown): void;
}

export interface NativeCubeGeometryCoordinatorOptions {
  document: Document;
  nodes: CubeNodeCatalog;
  graphChanges: CanvasGraphChangeSource;
  getCurrentGraph(): object | null;
  getRenderer(): NodeRenderer;
  scheduler: NativeCubeGeometryScheduler;
  logger?: Pick<Console, 'debug' | 'warn'>;
}

interface NativeCubeGeometryContext {
  cube: CubeNode;
  graph: CubeNode['subgraph'];
  nodes: ReadonlyMap<string, ComfyNode>;
}

/** Measure mounted native node cards once and restore their authored relations. */
export class NativeCubeGeometryCoordinator {
  readonly #document: Document;
  readonly #nodes: CubeNodeCatalog;
  readonly #getCurrentGraph: () => object | null;
  readonly #getRenderer: () => NodeRenderer;
  readonly #scheduler: NativeCubeGeometryScheduler;
  readonly #logger: Pick<Console, 'debug' | 'warn'> | null;
  readonly #unsubscribeGraphChanges: () => void;
  readonly #unsubscribeNodes: () => void;
  readonly #stabilized = new WeakSet<object>();
  #generation = 0;
  #scheduledHandle: unknown = null;

  /** Bind native Cube discovery to Comfy's explicit graph-navigation boundary. */
  constructor(options: NativeCubeGeometryCoordinatorOptions) {
    this.#document = options.document;
    this.#nodes = options.nodes;
    this.#getCurrentGraph = options.getCurrentGraph;
    this.#getRenderer = options.getRenderer;
    this.#scheduler = options.scheduler;
    this.#logger = options.logger ?? null;
    this.#unsubscribeGraphChanges = options.graphChanges.subscribe(() => this.refresh());
    this.#unsubscribeNodes = options.nodes.subscribe(() => this.refresh());
    this.refresh();
  }

  /** Cancel pending measurement and release graph and catalog subscriptions. */
  dispose(): void {
    this.#generation += 1;
    this.#cancelScheduled();
    this.#unsubscribeGraphChanges();
    this.#unsubscribeNodes();
  }

  /** Schedule stabilization when the active graph is an unstabilized Cube definition. */
  refresh(): void {
    this.#generation += 1;
    this.#cancelScheduled();
    const context = this.#readCurrentContext();
    if (!context || this.#stabilized.has(context.graph)) return;
    this.#attemptMount(context, this.#generation, 0);
  }

  /** Wait for Vue cards without delaying LiteGraph's canvas-owned measurement. */
  #attemptMount(context: NativeCubeGeometryContext, generation: number, attempt: number): void {
    if (!this.#isCurrent(context, generation)) return;
    const renderer = this.#getRenderer();
    const mounted =
      renderer !== 'vue' ||
      [...context.nodes.values()].every((node) => hasMountedNodePresentation(node, this.#document));
    if (!mounted && attempt + 1 < MAX_MOUNT_ATTEMPTS) {
      this.#scheduledHandle = this.#scheduler.schedule(
        () => this.#attemptMount(context, generation, attempt + 1),
        MOUNT_RETRY_DELAY_MS,
      );
      return;
    }
    if (!mounted) {
      this.#logger?.warn('SugarCubes native Cube geometry mount did not settle', {
        cubeId: context.cube.id,
        internalNodeCount: context.nodes.size,
      });
    }
    this.#reflow(context, renderer, mounted ? this.#document : null);
    if (!mounted) return;
    this.#scheduleVerification(context, generation, 0);
  }

  /** Re-measure after Vue finishes asynchronous widget and card layout. */
  #scheduleVerification(
    context: NativeCubeGeometryContext,
    generation: number,
    delayIndex: number,
  ): void {
    const delay = VERIFICATION_DELAYS_MS[delayIndex];
    if (delay === undefined) {
      this.#stabilized.add(context.graph);
      return;
    }
    this.#scheduledHandle = this.#scheduler.schedule(() => {
      if (!this.#isCurrent(context, generation)) return;
      this.#reflow(context, this.#getRenderer(), this.#document);
      this.#scheduleVerification(context, generation, delayIndex + 1);
    }, delay);
  }

  /** Apply one measured solve and invalidate only the active native definition. */
  #reflow(
    context: NativeCubeGeometryContext,
    renderer: NodeRenderer,
    document: Document | null,
  ): void {
    const baseline = readNativeCubeAuthoredLayout(context.graph);
    if (!baseline) return;
    const nodesChanged = applyMeasuredNodeGeometry(baseline, context.nodes, {
      renderer,
      document,
    });
    const boundariesChanged = applyNativeCubeBoundaryLayout(context.graph);
    if (!nodesChanged && !boundariesChanged) return;
    context.graph.setDirtyCanvas?.(true, true);
    this.#logger?.debug('SugarCubes native Cube geometry stabilized', {
      cubeId: context.cube.id,
      internalNodeCount: context.nodes.size,
    });
  }

  /** Resolve the active Cube and its stable internal symbols. */
  #readCurrentContext(): NativeCubeGeometryContext | null {
    const current = this.#getCurrentGraph();
    const cube = this.#nodes.list().find((candidate) => candidate.subgraph === current);
    if (!cube || !readNativeCubeAuthoredLayout(cube.subgraph)) return null;
    const nodes = new Map<string, ComfyNode>();
    for (const node of cube.subgraph._nodes) {
      const symbol =
        isRecord(node.properties) && typeof node.properties.sugarcubes_symbol === 'string'
          ? node.properties.sugarcubes_symbol.trim()
          : '';
      if (symbol) nodes.set(symbol, node);
    }
    return nodes.size ? { cube, graph: cube.subgraph, nodes } : null;
  }

  /** Reject callbacks that outlive navigation or a newer catalog generation. */
  #isCurrent(context: NativeCubeGeometryContext, generation: number): boolean {
    return generation === this.#generation && this.#getCurrentGraph() === context.graph;
  }

  /** Cancel the one owned scheduler handle without interpreting its host type. */
  #cancelScheduled(): void {
    if (this.#scheduledHandle === null) return;
    this.#scheduler.cancel(this.#scheduledHandle);
    this.#scheduledHandle = null;
  }
}
