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
/** Coordinate live renderer measurement from immutable cube layout baselines. */

import type { ComfyCanvas, ComfyGraph, ComfyNode } from '../types/graph.js';
import { applyMeasuredInstanceGeometry } from './ComfyInstanceGeometry.js';
import { applyComfyNodeSize, hasMountedNodePresentation } from './ComfyNodeGeometry.js';
import { collectLiveGeometryInstances } from './LiveCubeGeometryIndex.js';
import { resolveRendererGeometryPolicy, type NodeRenderer } from './RendererGeometryPolicy.js';

interface GeometryAdapter {
  getGraph?(): ComfyGraph | null;
  getCanvas?(): ComfyCanvas | null;
  getLiteGraph?(): LiteGraphHost | null;
  getNodeRenderer?(): NodeRenderer;
  getDocument?(): Document | null;
  getConsole?(): Console | null;
}

interface GeometryScheduler {
  raf(callback: FrameRequestCallback): number | null;
  timeout(callback: () => void, delayMs: number): number | null;
  clearTimeout(id: number): void;
}

interface GeometryCoordinatorOptions {
  adapter: GeometryAdapter;
  scheduler: GeometryScheduler;
  onStabilized?(graph: ComfyGraph): void;
  pollIntervalMs?: number;
}

const MAX_MOUNT_ATTEMPTS = 20;
const MOUNT_RETRY_DELAY_MS = 50;
const VERIFICATION_DELAYS_MS = [150, 500] as const;

/** Rebuild managed cube presentation from authored layout after host measurement. */
export class RendererGeometryCoordinator {
  private readonly adapter: GeometryAdapter;
  private readonly scheduler: GeometryScheduler;
  private readonly onStabilized: ((graph: ComfyGraph) => void) | null;
  private readonly pollIntervalMs: number;
  private renderer: NodeRenderer;
  private timerId: number | null;
  private active: boolean;
  private generation: number;

  constructor({
    adapter,
    scheduler,
    onStabilized,
    pollIntervalMs = 100,
  }: GeometryCoordinatorOptions) {
    this.adapter = adapter;
    this.scheduler = scheduler;
    this.onStabilized = onStabilized ?? null;
    this.pollIntervalMs = pollIntervalMs;
    this.renderer = resolveRendererGeometryPolicy(
      adapter.getLiteGraph?.(),
      adapter.getNodeRenderer?.(),
    ).renderer;
    this.timerId = null;
    this.active = false;
    this.generation = 0;
  }

  /** Start renderer observation without treating current live rectangles as authored data. */
  setup(): void {
    if (this.active) return;
    this.active = true;
    this.schedulePoll();
  }

  /** Stop renderer observation and invalidate queued measurement passes. */
  dispose(): void {
    this.active = false;
    this.generation += 1;
    if (this.timerId != null) this.scheduler.clearTimeout(this.timerId);
    this.timerId = null;
  }

  /** Stabilize newly imported instances after the active renderer mounts their nodes. */
  scheduleStabilization(_nodes: Iterable<ComfyNode>): void {
    this.scheduleReflow();
  }

  /** Check renderer state once; exposed for deterministic lifecycle tests. */
  checkRenderer(): void {
    const nextRenderer = resolveRendererGeometryPolicy(
      this.adapter.getLiteGraph?.(),
      this.adapter.getNodeRenderer?.(),
    ).renderer;
    if (nextRenderer === this.renderer) return;
    this.renderer = nextRenderer;
    this.scheduleReflow();
  }

  private schedulePoll(): void {
    if (!this.active) return;
    this.timerId = this.scheduler.timeout(() => {
      this.timerId = null;
      this.checkRenderer();
      this.schedulePoll();
    }, this.pollIntervalMs);
  }

  private scheduleReflow(): void {
    const graph = this.adapter.getGraph?.();
    if (!graph) return;
    const generation = ++this.generation;
    const policy = resolveRendererGeometryPolicy(
      this.adapter.getLiteGraph?.(),
      this.adapter.getNodeRenderer?.(),
    );
    const instances = collectLiveGeometryInstances(graph);
    for (const instance of instances) {
      for (const [identity, authored] of Object.entries(instance.baseline.entries)) {
        const node = instance.nodes.get(identity);
        if (node)
          applyComfyNodeSize(node, [authored.w, authored.h], policy, this.adapter.getConsole?.());
      }
    }
    this.scheduleMountedReflow(graph, policy.renderer, generation, instances.length, 0);
  }

  private scheduleMountedReflow(
    graph: ComfyGraph,
    renderer: NodeRenderer,
    generation: number,
    expectedInstanceCount: number,
    attempt: number,
  ): void {
    const inspect = () => {
      if (generation !== this.generation) return;
      const instances = collectLiveGeometryInstances(graph);
      const documentRef = this.adapter.getDocument?.() ?? null;
      const graphReady = instances.length >= expectedInstanceCount;
      const rendererReady =
        renderer !== 'vue' ||
        instances.every(({ nodes }) =>
          [...nodes.values()].every((node) => hasMountedNodePresentation(node, documentRef)),
        );
      if (graphReady && rendererReady) {
        this.reflow(graph, renderer);
        this.scheduleVerification(graph, renderer, generation, 0);
        return;
      }
      if (attempt + 1 < MAX_MOUNT_ATTEMPTS) {
        this.scheduleMountedReflow(graph, renderer, generation, expectedInstanceCount, attempt + 1);
        return;
      }
      this.adapter.getConsole?.()?.warn?.('SugarCubes geometry mount did not settle', {
        renderer,
        expectedInstanceCount,
        mountedInstanceCount: instances.length,
      });
      this.reflow(graph, renderer);
    };
    const scheduled = this.scheduler.timeout(inspect, MOUNT_RETRY_DELAY_MS);
    if (scheduled == null) this.afterFrames(2, inspect);
  }

  private scheduleVerification(
    graph: ComfyGraph,
    renderer: NodeRenderer,
    generation: number,
    delayIndex: number,
  ): void {
    const delay = VERIFICATION_DELAYS_MS[delayIndex];
    if (delay == null) return;
    const verify = () => {
      if (generation !== this.generation) return;
      this.reflow(graph, renderer);
      this.scheduleVerification(graph, renderer, generation, delayIndex + 1);
    };
    const scheduled = this.scheduler.timeout(verify, delay);
    if (scheduled == null) this.afterFrames(2, verify);
  }

  private afterFrames(remaining: number, callback: () => void): void {
    if (remaining <= 0) {
      callback();
      return;
    }
    const scheduled = this.scheduler.raf(() => this.afterFrames(remaining - 1, callback));
    if (scheduled == null) this.afterFrames(remaining - 1, callback);
  }

  private reflow(graph: ComfyGraph, renderer: NodeRenderer): void {
    let changed = false;
    const instances = collectLiveGeometryInstances(graph);
    for (const instance of instances) {
      changed =
        applyMeasuredInstanceGeometry(instance, {
          renderer,
          document: this.adapter.getDocument?.() ?? null,
        }) || changed;
    }
    if (!changed) return;
    graph.setDirtyCanvas?.(true, true);
    this.adapter.getCanvas?.()?.setDirty?.(true, true);
    this.adapter.getConsole?.()?.debug?.('SugarCubes renderer geometry stabilized', {
      renderer,
      instanceCount: instances.length,
    });
    this.onStabilized?.(graph);
  }
}
