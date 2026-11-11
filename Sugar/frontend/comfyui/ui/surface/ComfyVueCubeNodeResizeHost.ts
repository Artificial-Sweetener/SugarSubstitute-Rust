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
/** Supplement Comfy's native Nodes 2.0 corners with real-node edge resizing. */

import { resizeCubeFrame, type CubeResizeEdge } from '../cube/geometry/CubeResizeGeometry.js';
import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { Vec2 } from '../types/common.js';
import type { LiteGraphCubeNodeInteractionHistory } from './ComfyLiteGraphCubeNodeInteraction.js';
import { CUBE_BASE_MINIMUM_HEIGHT, cubeMinimumSize } from './CubeSurfaceMinimumHeight.js';

const EDGE_DIRECTIONS = ['n', 'e', 's', 'w'] as const satisfies readonly CubeResizeEdge[];

interface ResizeSession {
  input: 'pointer' | 'mouse';
  pointerId: number;
  edge: (typeof EDGE_DIRECTIONS)[number];
  startClient: Vec2;
  startPosition: Vec2;
  startSize: Vec2;
}

export interface ComfyVueCubeNodeResizeHostOptions {
  root: HTMLElement;
  node: CubeNode;
  history: LiteGraphCubeNodeInteractionHistory;
  getScale(): number;
  onGeometryChange?(): void;
}

/** Own only the four edge gestures absent from Comfy's native corner handles. */
export class ComfyVueCubeNodeResizeHost {
  readonly #root: HTMLElement;
  readonly #node: CubeNode;
  readonly #history: LiteGraphCubeNodeInteractionHistory;
  readonly #getScale: () => number;
  readonly #onGeometryChange: () => void;
  readonly #events: Window;
  readonly #handles: HTMLDivElement[] = [];
  #session: ResizeSession | null = null;
  #minimumHeight = CUBE_BASE_MINIMUM_HEIGHT;

  /** Add transparent edge hit targets while leaving native corners untouched. */
  constructor(options: ComfyVueCubeNodeResizeHostOptions) {
    this.#root = options.root;
    this.#node = options.node;
    this.#history = options.history;
    this.#getScale = options.getScale;
    this.#onGeometryChange = options.onGeometryChange ?? (() => undefined);
    const events = this.#root.ownerDocument.defaultView;
    if (!events) throw new Error('Cube edge resizing requires an active browser window.');
    this.#events = events;
    for (const edge of EDGE_DIRECTIONS) {
      const handle = this.#root.ownerDocument.createElement('div');
      handle.className = `sugarcubes-cube-node-edge-resize sugarcubes-cube-node-edge-resize--${edge}`;
      handle.dataset.sugarcubeEdgeResize = edge;
      handle.setAttribute('role', 'button');
      handle.setAttribute('aria-label', edgeLabel(edge));
      handle.addEventListener('pointerdown', this.#handlePointerDown, true);
      handle.addEventListener('mousedown', this.#handleMouseDown, true);
      this.#handles.push(handle);
    }
    this.ensureMounted();
  }

  /** Reattach edge handles if a native Vue patch replaces root children. */
  ensureMounted(): void {
    for (const handle of this.#handles) {
      if (handle.parentElement !== this.#root) this.#root.append(handle);
    }
  }

  /** Apply the latest measured face height to subsequent resize gestures. */
  setMinimumHeight(minimumHeight: number): void {
    this.#minimumHeight = cubeMinimumSize(minimumHeight)[1];
  }

  /** Release edge handles and close an unfinished history transaction once. */
  dispose(): void {
    if (this.#session) this.#history.afterChange?.();
    this.#detachSessionListeners();
    this.#session = null;
    for (const handle of this.#handles) {
      handle.removeEventListener('pointerdown', this.#handlePointerDown, true);
      handle.removeEventListener('mousedown', this.#handleMouseDown, true);
      handle.remove();
    }
    this.#handles.length = 0;
  }

  /** Begin one edge resize against the actual graph node. */
  readonly #handlePointerDown = (event: PointerEvent): void => {
    this.#start(event, 'pointer', event.pointerId);
    if (this.#session?.input === 'pointer') {
      this.#attachSessionListeners('pointer');
      this.#resolveHandle(event.target)?.setPointerCapture?.(event.pointerId);
    }
  };

  /** Support hosts that deliver mouse gestures without compatibility pointer events. */
  readonly #handleMouseDown = (event: MouseEvent): void => {
    if (this.#session?.input === 'pointer') {
      consume(event);
      return;
    }
    this.#start(event, 'mouse', 0);
    if (this.#session?.input === 'mouse') this.#attachSessionListeners('mouse');
  };

  /** Listen globally only for the duration of one active edge gesture. */
  #attachSessionListeners(input: ResizeSession['input']): void {
    if (input === 'pointer') {
      this.#events.addEventListener('pointermove', this.#handlePointerMove, true);
      this.#events.addEventListener('pointerup', this.#handlePointerUp, true);
      this.#events.addEventListener('pointercancel', this.#handlePointerCancel, true);
      return;
    }
    this.#events.addEventListener('mousemove', this.#handleMouseMove, true);
    this.#events.addEventListener('mouseup', this.#handleMouseUp, true);
  }

  /** Remove both input variants idempotently at finish or disposal. */
  #detachSessionListeners(): void {
    this.#events.removeEventListener('pointermove', this.#handlePointerMove, true);
    this.#events.removeEventListener('pointerup', this.#handlePointerUp, true);
    this.#events.removeEventListener('pointercancel', this.#handlePointerCancel, true);
    this.#events.removeEventListener('mousemove', this.#handleMouseMove, true);
    this.#events.removeEventListener('mouseup', this.#handleMouseUp, true);
  }

  /** Begin one resize session from a native event target. */
  #start(event: MouseEvent | PointerEvent, input: ResizeSession['input'], pointerId: number): void {
    if (event.button !== 0 || this.#session) return;
    const target = this.#resolveHandle(event.target);
    if (!target) return;
    const edge = readEdge(target.dataset.sugarcubeEdgeResize);
    if (!edge) return;
    consume(event);
    this.#history.beforeChange?.();
    this.#session = {
      input,
      pointerId,
      edge,
      startClient: [event.clientX, event.clientY],
      startPosition: [Number(this.#node.pos[0]), Number(this.#node.pos[1])],
      startSize: [Number(this.#node.size[0]), Number(this.#node.size[1])],
    };
  }

  /** Resolve only this Cube node's supplemental edge targets. */
  #resolveHandle(target: EventTarget | null): HTMLDivElement | null {
    if (!(target instanceof HTMLElement)) return null;
    const handle = target.closest<HTMLDivElement>('[data-sugarcube-edge-resize]');
    return handle && this.#handles.includes(handle) ? handle : null;
  }

  /** Apply viewport movement in graph units through the native node geometry. */
  readonly #handlePointerMove = (event: PointerEvent): void => {
    const session = this.#session;
    if (!session || session.input !== 'pointer' || session.pointerId !== event.pointerId) return;
    this.#applyMove(event, session);
  };

  /** Resize when the browser host emits only mouse movement. */
  readonly #handleMouseMove = (event: MouseEvent): void => {
    const session = this.#session;
    if (!session) return;
    if (session.input === 'pointer') {
      consume(event);
      return;
    }
    this.#applyMove(event, session);
  };

  /** Apply one viewport delta to real native-node geometry. */
  #applyMove(event: MouseEvent | PointerEvent, session: ResizeSession): void {
    consume(event);
    const scale = finiteScale(this.#getScale());
    const frame = resizeCubeFrame({
      edge: session.edge,
      startPosition: session.startPosition,
      startSize: session.startSize,
      delta: [
        (event.clientX - session.startClient[0]) / scale,
        (event.clientY - session.startClient[1]) / scale,
      ],
      minimumSize: cubeMinimumSize(this.#minimumHeight),
    });
    setNodePosition(this.#node, frame.position);
    this.#node.setSize?.([...frame.size]);
    writePair(this.#node.size, frame.size);
    this.#node.onResize?.([...frame.size]);
    this.#onGeometryChange();
    this.#history.setDirtyCanvas?.(true, true);
  }

  /** Complete one native-node history transaction. */
  readonly #handlePointerUp = (event: PointerEvent): void => {
    if (this.#session?.input === 'pointer') this.#finish(event, event.pointerId);
  };

  /** Cancel one edge gesture without leaving a pending transaction. */
  readonly #handlePointerCancel = (event: PointerEvent): void => {
    if (this.#session?.input === 'pointer') this.#finish(event, event.pointerId);
  };

  /** Finish a mouse-only resize transaction. */
  readonly #handleMouseUp = (event: MouseEvent): void => {
    if (this.#session?.input === 'mouse') this.#finish(event, 0);
  };

  /** End one matching pointer session. */
  #finish(event: MouseEvent | PointerEvent, pointerId: number): void {
    if (!this.#session || this.#session.pointerId !== pointerId) return;
    consume(event);
    this.#detachSessionListeners();
    this.#session = null;
    this.#history.afterChange?.();
    this.#history.setDirtyCanvas?.(true, true);
  }
}

/** Read only the edge directions this host owns. */
function readEdge(value: string | undefined): ResizeSession['edge'] | null {
  return EDGE_DIRECTIONS.includes(value as ResizeSession['edge'])
    ? (value as ResizeSession['edge'])
    : null;
}

/** Normalize graph zoom without allowing a divide-by-zero gesture. */
function finiteScale(value: number): number {
  return Number.isFinite(value) && value > 0 ? value : 1;
}

/** Write a host-owned numeric vector without replacing its representation. */
function writePair(target: ArrayLike<number> & Record<number, number>, value: Vec2): void {
  target[0] = value[0];
  target[1] = value[1];
}

/** Notify Comfy's Nodes 2.0 layout store when anchored resizing moves the node origin. */
function setNodePosition(node: CubeNode, position: Vec2): void {
  if (node.setPos) {
    node.setPos(position[0], position[1]);
    return;
  }
  node.pos = [...position];
}

/** Name one supplemental edge without imitating Comfy's visual controls. */
function edgeLabel(edge: ResizeSession['edge']): string {
  const labels: Record<ResizeSession['edge'], string> = {
    n: 'Resize Cube from top edge',
    e: 'Resize Cube from right edge',
    s: 'Resize Cube from bottom edge',
    w: 'Resize Cube from left edge',
  };
  return labels[edge];
}

/** Prevent native movement from competing with a Cube-owned edge gesture. */
function consume(event: Event): void {
  event.preventDefault();
  event.stopPropagation();
  event.stopImmediatePropagation();
}
