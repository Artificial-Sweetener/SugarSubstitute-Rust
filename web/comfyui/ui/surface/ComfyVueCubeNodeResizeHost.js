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
import { resizeCubeFrame } from '../cube/geometry/CubeResizeGeometry.js';
import { CUBE_BASE_MINIMUM_HEIGHT, cubeMinimumSize } from './CubeSurfaceMinimumHeight.js';
const EDGE_DIRECTIONS = ['n', 'e', 's', 'w'];
/** Own only the four edge gestures absent from Comfy's native corner handles. */
export class ComfyVueCubeNodeResizeHost {
    #root;
    #node;
    #history;
    #getScale;
    #onGeometryChange;
    #events;
    #handles = [];
    #session = null;
    #minimumHeight = CUBE_BASE_MINIMUM_HEIGHT;
    /** Add transparent edge hit targets while leaving native corners untouched. */
    constructor(options) {
        this.#root = options.root;
        this.#node = options.node;
        this.#history = options.history;
        this.#getScale = options.getScale;
        this.#onGeometryChange = options.onGeometryChange ?? (() => undefined);
        const events = this.#root.ownerDocument.defaultView;
        if (!events)
            throw new Error('Cube edge resizing requires an active browser window.');
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
    ensureMounted() {
        for (const handle of this.#handles) {
            if (handle.parentElement !== this.#root)
                this.#root.append(handle);
        }
    }
    /** Apply the latest measured face height to subsequent resize gestures. */
    setMinimumHeight(minimumHeight) {
        this.#minimumHeight = cubeMinimumSize(minimumHeight)[1];
    }
    /** Release edge handles and close an unfinished history transaction once. */
    dispose() {
        if (this.#session)
            this.#history.afterChange?.();
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
    #handlePointerDown = (event) => {
        this.#start(event, 'pointer', event.pointerId);
        if (this.#session?.input === 'pointer') {
            this.#attachSessionListeners('pointer');
            this.#resolveHandle(event.target)?.setPointerCapture?.(event.pointerId);
        }
    };
    /** Support hosts that deliver mouse gestures without compatibility pointer events. */
    #handleMouseDown = (event) => {
        if (this.#session?.input === 'pointer') {
            consume(event);
            return;
        }
        this.#start(event, 'mouse', 0);
        if (this.#session?.input === 'mouse')
            this.#attachSessionListeners('mouse');
    };
    /** Listen globally only for the duration of one active edge gesture. */
    #attachSessionListeners(input) {
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
    #detachSessionListeners() {
        this.#events.removeEventListener('pointermove', this.#handlePointerMove, true);
        this.#events.removeEventListener('pointerup', this.#handlePointerUp, true);
        this.#events.removeEventListener('pointercancel', this.#handlePointerCancel, true);
        this.#events.removeEventListener('mousemove', this.#handleMouseMove, true);
        this.#events.removeEventListener('mouseup', this.#handleMouseUp, true);
    }
    /** Begin one resize session from a native event target. */
    #start(event, input, pointerId) {
        if (event.button !== 0 || this.#session)
            return;
        const target = this.#resolveHandle(event.target);
        if (!target)
            return;
        const edge = readEdge(target.dataset.sugarcubeEdgeResize);
        if (!edge)
            return;
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
    #resolveHandle(target) {
        if (!(target instanceof HTMLElement))
            return null;
        const handle = target.closest('[data-sugarcube-edge-resize]');
        return handle && this.#handles.includes(handle) ? handle : null;
    }
    /** Apply viewport movement in graph units through the native node geometry. */
    #handlePointerMove = (event) => {
        const session = this.#session;
        if (!session || session.input !== 'pointer' || session.pointerId !== event.pointerId)
            return;
        this.#applyMove(event, session);
    };
    /** Resize when the browser host emits only mouse movement. */
    #handleMouseMove = (event) => {
        const session = this.#session;
        if (!session)
            return;
        if (session.input === 'pointer') {
            consume(event);
            return;
        }
        this.#applyMove(event, session);
    };
    /** Apply one viewport delta to real native-node geometry. */
    #applyMove(event, session) {
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
    #handlePointerUp = (event) => {
        if (this.#session?.input === 'pointer')
            this.#finish(event, event.pointerId);
    };
    /** Cancel one edge gesture without leaving a pending transaction. */
    #handlePointerCancel = (event) => {
        if (this.#session?.input === 'pointer')
            this.#finish(event, event.pointerId);
    };
    /** Finish a mouse-only resize transaction. */
    #handleMouseUp = (event) => {
        if (this.#session?.input === 'mouse')
            this.#finish(event, 0);
    };
    /** End one matching pointer session. */
    #finish(event, pointerId) {
        if (!this.#session || this.#session.pointerId !== pointerId)
            return;
        consume(event);
        this.#detachSessionListeners();
        this.#session = null;
        this.#history.afterChange?.();
        this.#history.setDirtyCanvas?.(true, true);
    }
}
/** Read only the edge directions this host owns. */
function readEdge(value) {
    return EDGE_DIRECTIONS.includes(value)
        ? value
        : null;
}
/** Normalize graph zoom without allowing a divide-by-zero gesture. */
function finiteScale(value) {
    return Number.isFinite(value) && value > 0 ? value : 1;
}
/** Write a host-owned numeric vector without replacing its representation. */
function writePair(target, value) {
    target[0] = value[0];
    target[1] = value[1];
}
/** Notify Comfy's Nodes 2.0 layout store when anchored resizing moves the node origin. */
function setNodePosition(node, position) {
    if (node.setPos) {
        node.setPos(position[0], position[1]);
        return;
    }
    node.pos = [...position];
}
/** Name one supplemental edge without imitating Comfy's visual controls. */
function edgeLabel(edge) {
    const labels = {
        n: 'Resize Cube from top edge',
        e: 'Resize Cube from right edge',
        s: 'Resize Cube from bottom edge',
        w: 'Resize Cube from left edge',
    };
    return labels[edge];
}
/** Prevent native movement from competing with a Cube-owned edge gesture. */
function consume(event) {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation();
}
