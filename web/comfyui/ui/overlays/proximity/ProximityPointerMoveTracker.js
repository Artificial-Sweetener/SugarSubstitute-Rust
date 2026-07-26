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
/** Refresh proximity geometry only when pointer interaction changes node geometry. */
import { getGraphNodes } from '../../graph/GraphQuery.js';
/** Own the DOM movement signal that LiteGraph canvas callbacks do not receive. */
export class ProximityPointerMoveTracker {
    #proximity;
    #scheduler;
    #attached = new WeakSet();
    #geometry = new WeakMap();
    #lastMoved = new WeakMap();
    #settling = new WeakSet();
    /** Bind the renderer-neutral proximity preview owner. */
    constructor(proximity, scheduler = null) {
        this.#proximity = proximity;
        this.#scheduler = scheduler;
    }
    /** Attach one idempotent movement listener to the active graph surface. */
    attach(element, surface) {
        const eventTarget = element.closest('.graph-canvas-container') ?? element.parentElement ?? element;
        if (this.#attached.has(eventTarget))
            return;
        this.#snapshot(surface.graph);
        const refresh = (event) => {
            if (!this.#proximity.isProximityEnabled())
                return;
            const node = resolveMovedNode(event, surface);
            if (!node || !this.#geometryChanged(node))
                return;
            this.#lastMoved.set(eventTarget, node);
            this.#proximity.schedulePreview({ graph: surface.graph });
        };
        const settle = () => {
            if (!this.#proximity.isProximityEnabled() || !this.#lastMoved.has(eventTarget))
                return;
            if (this.#settling.has(eventTarget))
                return;
            this.#settling.add(eventTarget);
            this.#afterFrames(() => {
                this.#settling.delete(eventTarget);
                this.#proximity.schedulePreview({ graph: surface.graph });
            }, 2);
        };
        eventTarget.addEventListener('pointermove', refresh);
        eventTarget.addEventListener('mousemove', refresh);
        eventTarget.addEventListener('pointerup', settle);
        eventTarget.addEventListener('mouseup', settle);
        this.#attached.add(eventTarget);
    }
    /** Refresh after DOM and canvas drag owners commit their final graph position. */
    #afterFrames(callback, remainingFrames) {
        if (remainingFrames <= 0 || typeof this.#scheduler?.raf !== 'function') {
            callback();
            return;
        }
        this.#scheduler.raf(() => this.#afterFrames(callback, remainingFrames - 1));
    }
    /** Record the graph's initial geometry once so ordinary hovering remains free. */
    #snapshot(graph) {
        for (const node of getGraphNodes(graph)) {
            this.#geometry.set(node, readGeometry(node));
        }
    }
    /** Return true only for a real position or size transition. */
    #geometryChanged(node) {
        const next = readGeometry(node);
        const previous = this.#geometry.get(node);
        this.#geometry.set(node, next);
        return (!previous ||
            previous.x !== next.x ||
            previous.y !== next.y ||
            previous.width !== next.width ||
            previous.height !== next.height);
    }
}
/** Resolve the one node owned by a native or DOM drag event. */
function resolveMovedNode(event, surface) {
    if (surface.node_dragged)
        return surface.node_dragged;
    const target = event.target;
    if (!(target instanceof Element))
        return null;
    const nodeRoot = target.closest('.lg-node[data-node-id]');
    const nodeId = nodeRoot?.dataset.nodeId;
    return nodeId ? findNode(surface.graph, nodeId) : null;
}
/** Find one node without assuming Comfy's concrete graph implementation. */
function findNode(graph, nodeId) {
    const getNodeById = graph.getNodeById;
    if (typeof getNodeById === 'function') {
        const node = getNodeById.call(graph, nodeId);
        if (node && typeof node === 'object')
            return node;
    }
    return getGraphNodes(graph).find((node) => String(node.id) === String(nodeId)) ?? null;
}
/** Normalize external vector values into a stable comparison record. */
function readGeometry(node) {
    return {
        x: finite(node.pos?.[0]),
        y: finite(node.pos?.[1]),
        width: finite(node.size?.[0]),
        height: finite(node.size?.[1]),
    };
}
/** Keep malformed host coordinates from creating perpetual false changes. */
function finite(value) {
    const number = Number(value);
    return Number.isFinite(number) ? number : 0;
}
