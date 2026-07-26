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
import type { ComfyGraph, ComfyNode, GraphId } from '../../types/graph.js';

interface ProximityPreview {
  isProximityEnabled(): boolean;
  schedulePreview(options: { graph: ComfyGraph }): void;
}

interface FrameScheduler {
  raf?(callback: FrameRequestCallback): number | null;
}

interface GraphSurface {
  graph: ComfyGraph;
  node_dragged?: ComfyNode | null;
}

interface NodeGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Own the DOM movement signal that LiteGraph canvas callbacks do not receive. */
export class ProximityPointerMoveTracker {
  readonly #proximity: ProximityPreview;
  readonly #scheduler: FrameScheduler | null;
  readonly #attached = new WeakSet<EventTarget>();
  readonly #geometry = new WeakMap<ComfyNode, NodeGeometry>();
  readonly #lastMoved = new WeakMap<EventTarget, ComfyNode>();
  readonly #settling = new WeakSet<EventTarget>();

  /** Bind the renderer-neutral proximity preview owner. */
  constructor(proximity: ProximityPreview, scheduler: FrameScheduler | null = null) {
    this.#proximity = proximity;
    this.#scheduler = scheduler;
  }

  /** Attach one idempotent movement listener to the active graph surface. */
  attach(element: HTMLCanvasElement, surface: GraphSurface): void {
    const eventTarget =
      element.closest('.graph-canvas-container') ?? element.parentElement ?? element;
    if (this.#attached.has(eventTarget)) return;
    this.#snapshot(surface.graph);
    const refresh = (event: Event): void => {
      if (!this.#proximity.isProximityEnabled()) return;
      const node = resolveMovedNode(event, surface);
      if (!node || !this.#geometryChanged(node)) return;
      this.#lastMoved.set(eventTarget, node);
      this.#proximity.schedulePreview({ graph: surface.graph });
    };
    const settle = (): void => {
      if (!this.#proximity.isProximityEnabled() || !this.#lastMoved.has(eventTarget)) return;
      if (this.#settling.has(eventTarget)) return;
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
  #afterFrames(callback: () => void, remainingFrames: number): void {
    if (remainingFrames <= 0 || typeof this.#scheduler?.raf !== 'function') {
      callback();
      return;
    }
    this.#scheduler.raf(() => this.#afterFrames(callback, remainingFrames - 1));
  }

  /** Record the graph's initial geometry once so ordinary hovering remains free. */
  #snapshot(graph: ComfyGraph): void {
    for (const node of getGraphNodes(graph)) {
      this.#geometry.set(node, readGeometry(node));
    }
  }

  /** Return true only for a real position or size transition. */
  #geometryChanged(node: ComfyNode): boolean {
    const next = readGeometry(node);
    const previous = this.#geometry.get(node);
    this.#geometry.set(node, next);
    return (
      !previous ||
      previous.x !== next.x ||
      previous.y !== next.y ||
      previous.width !== next.width ||
      previous.height !== next.height
    );
  }
}

/** Resolve the one node owned by a native or DOM drag event. */
function resolveMovedNode(event: Event, surface: GraphSurface): ComfyNode | null {
  if (surface.node_dragged) return surface.node_dragged;
  const target = event.target;
  if (!(target instanceof Element)) return null;
  const nodeRoot = target.closest<HTMLElement>('.lg-node[data-node-id]');
  const nodeId = nodeRoot?.dataset.nodeId;
  return nodeId ? findNode(surface.graph, nodeId) : null;
}

/** Find one node without assuming Comfy's concrete graph implementation. */
function findNode(graph: ComfyGraph, nodeId: GraphId): ComfyNode | null {
  const getNodeById = graph.getNodeById;
  if (typeof getNodeById === 'function') {
    const node: unknown = getNodeById.call(graph, nodeId);
    if (node && typeof node === 'object') return node as ComfyNode;
  }
  return getGraphNodes(graph).find((node) => String(node.id) === String(nodeId)) ?? null;
}

/** Normalize external vector values into a stable comparison record. */
function readGeometry(node: ComfyNode): NodeGeometry {
  return {
    x: finite(node.pos?.[0]),
    y: finite(node.pos?.[1]),
    width: finite(node.size?.[0]),
    height: finite(node.size?.[1]),
  };
}

/** Keep malformed host coordinates from creating perpetual false changes. */
function finite(value: unknown): number {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}
