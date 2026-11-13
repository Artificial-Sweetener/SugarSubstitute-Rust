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
 * Refresh proximity ownership after authoritative LiteGraph mutations.
 */

import type { ComfyGraph } from '../../types/graph.js';

type GraphMutationCallback = (this: ComfyGraph, ...args: unknown[]) => unknown;
type GraphMutationKey =
  | 'onNodeAdded'
  | 'onNodeRemoved'
  | 'onConnectionChange'
  | 'onNodeConnectionChange';

interface ProximityPreviewOwner {
  isProximityEnabled(): boolean;
  schedulePreview(options: { graph: ComfyGraph }): void;
}

interface FrameScheduler {
  raf?(callback: FrameRequestCallback): number | null;
}

interface MutationAwareGraph extends ComfyGraph {
  __sugarcubes_proximity_mutation_wrapped?: boolean;
  onNodeAdded?: GraphMutationCallback;
  onNodeRemoved?: GraphMutationCallback;
  onConnectionChange?: GraphMutationCallback;
  onNodeConnectionChange?: GraphMutationCallback;
  removeLink?: GraphMutationCallback;
}

/**
 * Observe graph mutations that can change proximity candidates or noodle ownership.
 */
export class ProximityGraphMutationTracker {
  readonly #proximity: ProximityPreviewOwner;
  readonly #scheduler: FrameScheduler | null;
  readonly #pendingOwnershipRefresh = new WeakSet<ComfyGraph>();

  /**
   * Create a graph mutation tracker.
   */
  constructor(proximity: ProximityPreviewOwner, scheduler: FrameScheduler | null) {
    this.#proximity = proximity;
    this.#scheduler = scheduler;
  }

  /**
   * Attach idempotent mutation hooks to one LiteGraph graph.
   */
  attach(graphValue: ComfyGraph | null | undefined): void {
    const graph = graphValue as MutationAwareGraph | null | undefined;
    if (!graph || graph.__sugarcubes_proximity_mutation_wrapped) {
      return;
    }
    graph.__sugarcubes_proximity_mutation_wrapped = true;
    this.#wrapInventoryCallback(graph, 'onNodeAdded');
    this.#wrapInventoryCallback(graph, 'onNodeRemoved');
    this.#wrapOwnershipCallback(graph, 'onConnectionChange');
    this.#wrapOwnershipCallback(graph, 'onNodeConnectionChange');
    this.#wrapRemoveLink(graph);
  }

  /**
   * Refresh immediately after node inventory changes.
   */
  #wrapInventoryCallback(graph: MutationAwareGraph, key: GraphMutationKey): void {
    const original = graph[key];
    const tracker = this;
    graph[key] = function trackedInventoryMutation(this: ComfyGraph, ...args: unknown[]): unknown {
      const result = original?.call(this, ...args);
      tracker.#refreshNow(this);
      return result;
    };
  }

  /**
   * Refresh after LiteGraph finishes updating both ends of a link.
   */
  #wrapOwnershipCallback(graph: MutationAwareGraph, key: GraphMutationKey): void {
    const original = graph[key];
    const tracker = this;
    graph[key] = function trackedOwnershipMutation(this: ComfyGraph, ...args: unknown[]): unknown {
      const result = original?.call(this, ...args);
      tracker.#refreshAfterGraphSettles(this);
      return result;
    };
  }

  /**
   * Observe the authoritative context-menu and programmatic link-removal boundary.
   */
  #wrapRemoveLink(graph: MutationAwareGraph): void {
    const original = graph.removeLink;
    if (typeof original !== 'function') {
      return;
    }
    const tracker = this;
    graph.removeLink = function trackedRemoveLink(this: ComfyGraph, ...args: unknown[]): unknown {
      const result = original.call(this, ...args);
      tracker.#refreshAfterGraphSettles(this);
      return result;
    };
  }

  /**
   * Request a candidate refresh without creating work while proximity is disabled.
   */
  #refreshNow(graph: ComfyGraph): void {
    if (this.#proximity.isProximityEnabled()) {
      this.#proximity.schedulePreview({ graph });
    }
  }

  /**
   * Coalesce link callbacks and wait for LiteGraph's endpoint bookkeeping.
   */
  #refreshAfterGraphSettles(graph: ComfyGraph): void {
    if (this.#pendingOwnershipRefresh.has(graph)) {
      return;
    }
    this.#pendingOwnershipRefresh.add(graph);
    this.#afterFrames(() => {
      this.#pendingOwnershipRefresh.delete(graph);
      this.#refreshNow(graph);
    }, 2);
  }

  /**
   * Run after a bounded number of animation frames.
   */
  #afterFrames(callback: () => void, remainingFrames: number): void {
    if (remainingFrames <= 0 || typeof this.#scheduler?.raf !== 'function') {
      callback();
      return;
    }
    this.#scheduler.raf(() => this.#afterFrames(callback, remainingFrames - 1));
  }
}
