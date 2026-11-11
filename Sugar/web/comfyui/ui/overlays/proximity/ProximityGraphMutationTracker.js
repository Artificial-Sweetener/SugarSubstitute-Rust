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
/**
 * Observe graph mutations that can change proximity candidates or noodle ownership.
 */
export class ProximityGraphMutationTracker {
    #proximity;
    #scheduler;
    #pendingOwnershipRefresh = new WeakSet();
    /**
     * Create a graph mutation tracker.
     */
    constructor(proximity, scheduler) {
        this.#proximity = proximity;
        this.#scheduler = scheduler;
    }
    /**
     * Attach idempotent mutation hooks to one LiteGraph graph.
     */
    attach(graphValue) {
        const graph = graphValue;
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
    #wrapInventoryCallback(graph, key) {
        const original = graph[key];
        const tracker = this;
        graph[key] = function trackedInventoryMutation(...args) {
            const result = original?.call(this, ...args);
            tracker.#refreshNow(this);
            return result;
        };
    }
    /**
     * Refresh after LiteGraph finishes updating both ends of a link.
     */
    #wrapOwnershipCallback(graph, key) {
        const original = graph[key];
        const tracker = this;
        graph[key] = function trackedOwnershipMutation(...args) {
            const result = original?.call(this, ...args);
            tracker.#refreshAfterGraphSettles(this);
            return result;
        };
    }
    /**
     * Observe the authoritative context-menu and programmatic link-removal boundary.
     */
    #wrapRemoveLink(graph) {
        const original = graph.removeLink;
        if (typeof original !== 'function') {
            return;
        }
        const tracker = this;
        graph.removeLink = function trackedRemoveLink(...args) {
            const result = original.call(this, ...args);
            tracker.#refreshAfterGraphSettles(this);
            return result;
        };
    }
    /**
     * Request a candidate refresh without creating work while proximity is disabled.
     */
    #refreshNow(graph) {
        if (this.#proximity.isProximityEnabled()) {
            this.#proximity.schedulePreview({ graph });
        }
    }
    /**
     * Coalesce link callbacks and wait for LiteGraph's endpoint bookkeeping.
     */
    #refreshAfterGraphSettles(graph) {
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
    #afterFrames(callback, remainingFrames) {
        if (remainingFrames <= 0 || typeof this.#scheduler?.raf !== 'function') {
            callback();
            return;
        }
        this.#scheduler.raf(() => this.#afterFrames(callback, remainingFrames - 1));
    }
}
